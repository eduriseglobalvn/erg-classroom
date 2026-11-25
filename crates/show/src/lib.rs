//! Transport for the one-to-many screen Show: frames are split into UDP-sized shards, protected
//! with Reed-Solomon parity (so a few lost packets need no retransmission, which multicast cannot
//! do anyway) and encrypted with AES-128-GCM under a per-session key.
//!
//! Wire packet = HEADER (25 bytes, authenticated but not encrypted) | ciphertext | 16-byte tag.
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::BTreeMap;

pub mod udp;

pub const MAGIC: u16 = 0xE5C1;
pub const HEADER_LEN: usize = 25;
pub const TAG_LEN: usize = 16;
/// Payload per packet. 1100 + header + tag stays under the 1280-byte IPv6 minimum MTU, so packets are never fragmented.
pub const SHARD_LEN: usize = 1100;
/// Max data shards per Reed-Solomon group (GF(2^8) limits data + parity to 255).
const MAX_DATA: usize = 200;
const MAX_PENDING_FRAMES: usize = 8;

pub type SessionKey = [u8; 16];

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("malformed packet")]
    Malformed,
    #[error("authentication failed")]
    Auth,
    #[error("frame too large")]
    TooLarge,
}

#[derive(Debug, Clone, Copy)]
struct Header {
    keyframe: bool,
    stream: u32,
    frame: u32,
    frame_len: u32,
    group: u16,
    groups: u16,
    k: u8,
    m: u8,
    idx: u8,
    shard_len: u16,
}

impl Header {
    fn write(&self) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[0..2].copy_from_slice(&MAGIC.to_le_bytes());
        h[2] = 1;
        h[3] = self.keyframe as u8;
        h[4..8].copy_from_slice(&self.stream.to_le_bytes());
        h[8..12].copy_from_slice(&self.frame.to_le_bytes());
        h[12..16].copy_from_slice(&self.frame_len.to_le_bytes());
        h[16..18].copy_from_slice(&self.group.to_le_bytes());
        h[18..20].copy_from_slice(&self.groups.to_le_bytes());
        h[20] = self.k;
        h[21] = self.m;
        h[22] = self.idx;
        h[23..25].copy_from_slice(&self.shard_len.to_le_bytes());
        h
    }

    fn read(b: &[u8]) -> Result<Self, Error> {
        if b.len() < HEADER_LEN + TAG_LEN || u16::from_le_bytes([b[0], b[1]]) != MAGIC || b[2] != 1 {
            return Err(Error::Malformed);
        }
        let u32le = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let u16le = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        let h = Header {
            keyframe: b[3] & 1 == 1,
            stream: u32le(4),
            frame: u32le(8),
            frame_len: u32le(12),
            group: u16le(16),
            groups: u16le(18),
            k: b[20],
            m: b[21],
            idx: b[22],
            shard_len: u16le(23),
        };
        let ok = h.k >= 1
            && (h.k as usize) <= MAX_DATA
            && (h.k as usize + h.m as usize) <= 255
            && h.idx < h.k + h.m
            && h.groups >= 1
            && h.group < h.groups
            && h.shard_len as usize == b.len() - HEADER_LEN - TAG_LEN
            && h.shard_len as usize <= SHARD_LEN;
        if ok { Ok(h) } else { Err(Error::Malformed) }
    }

    /// 96-bit nonce: unique per (stream, frame, group, shard) so a key is never reused with the same nonce.
    fn nonce(&self) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[0..4].copy_from_slice(&self.stream.to_le_bytes());
        n[4..8].copy_from_slice(&self.frame.to_le_bytes());
        n[8..10].copy_from_slice(&self.group.to_le_bytes());
        n[10] = self.idx;
        n
    }
}

pub struct Sender {
    cipher: Aes128Gcm,
    stream: u32,
    /// Parity shards per 100 data shards (20 = survive roughly 1 lost packet in 6).
    parity_pct: usize,
}

impl Sender {
    pub fn new(key: &SessionKey, stream: u32, parity_pct: usize) -> Self {
        Self { cipher: Aes128Gcm::new(key.into()), stream, parity_pct }
    }

    pub fn set_parity_pct(&mut self, pct: usize) {
        self.parity_pct = pct.min(100);
    }

    /// Split one encoded frame into encrypted packets ready to send.
    pub fn packetize(&self, frame_no: u32, keyframe: bool, data: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        if data.len() > u32::MAX as usize / 2 {
            return Err(Error::TooLarge);
        }
        let total_shards = data.len().div_ceil(SHARD_LEN).max(1);
        let groups = total_shards.div_ceil(MAX_DATA);
        if groups > u16::MAX as usize {
            return Err(Error::TooLarge);
        }
        let mut out = Vec::new();
        for g in 0..groups {
            let chunk = &data[(g * MAX_DATA * SHARD_LEN).min(data.len())..((g + 1) * MAX_DATA * SHARD_LEN).min(data.len())];
            let k = chunk.len().div_ceil(SHARD_LEN).max(1);
            let m = if self.parity_pct == 0 { 0 } else { (k * self.parity_pct).div_ceil(100).max(1).min(255 - k) };
            let mut shards: Vec<Vec<u8>> = (0..k)
                .map(|i| {
                    let mut s = chunk[(i * SHARD_LEN).min(chunk.len())..((i + 1) * SHARD_LEN).min(chunk.len())].to_vec();
                    s.resize(SHARD_LEN, 0); // pad: the true length is recovered from frame_len
                    s
                })
                .collect();
            shards.extend((0..m).map(|_| vec![0u8; SHARD_LEN]));
            if m > 0 {
                ReedSolomon::new(k, m).map_err(|_| Error::TooLarge)?.encode(&mut shards).map_err(|_| Error::TooLarge)?;
            }
            for (idx, s) in shards.iter().enumerate() {
                let h = Header {
                    keyframe,
                    stream: self.stream,
                    frame: frame_no,
                    frame_len: data.len() as u32,
                    group: g as u16,
                    groups: groups as u16,
                    k: k as u8,
                    m: m as u8,
                    idx: idx as u8,
                    shard_len: SHARD_LEN as u16,
                };
                let hb = h.write();
                let ct = self
                    .cipher
                    .encrypt(Nonce::from_slice(&h.nonce()), Payload { msg: s, aad: &hb })
                    .map_err(|_| Error::Auth)?;
                let mut pkt = hb.to_vec();
                pkt.extend(ct);
                out.push(pkt);
            }
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub frame_no: u32,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub packets: u64,
    pub frames_ok: u64,
    /// Frames abandoned because newer data arrived before they could be rebuilt.
    pub frames_lost: u64,
    /// Frames that needed parity to be rebuilt.
    pub frames_fec_repaired: u64,
    pub rejected: u64,
}

struct Pending {
    keyframe: bool,
    frame_len: usize,
    groups: usize,
    /// per group: (k, m, shards by index)
    g: BTreeMap<u16, (usize, usize, Vec<Option<Vec<u8>>>)>,
    used_parity: bool,
}

pub struct Receiver {
    cipher: Aes128Gcm,
    stream: u32,
    pending: BTreeMap<u32, Pending>,
    last_done: Option<u32>,
    pub stats: Stats,
}

impl Receiver {
    pub fn new(key: &SessionKey, stream: u32) -> Self {
        Self { cipher: Aes128Gcm::new(key.into()), stream, pending: BTreeMap::new(), last_done: None, stats: Stats::default() }
    }

    /// Feed one received packet. Returns a frame once enough shards of every group have arrived.
    /// Bad, forged or foreign packets are counted and ignored.
    pub fn push(&mut self, pkt: &[u8]) -> Option<Frame> {
        match self.try_push(pkt) {
            Ok(f) => f,
            Err(_) => {
                self.stats.rejected += 1;
                None
            }
        }
    }

    fn try_push(&mut self, pkt: &[u8]) -> Result<Option<Frame>, Error> {
        let h = Header::read(pkt)?;
        if h.stream != self.stream {
            return Err(Error::Malformed);
        }
        if self.last_done.is_some_and(|d| h.frame <= d) {
            return Ok(None); // late packet of a frame we already delivered or gave up on
        }
        let shard = self
            .cipher
            .decrypt(Nonce::from_slice(&h.nonce()), Payload { msg: &pkt[HEADER_LEN..], aad: &pkt[..HEADER_LEN] })
            .map_err(|_| Error::Auth)?;
        self.stats.packets += 1;

        let p = self.pending.entry(h.frame).or_insert_with(|| Pending {
            keyframe: h.keyframe,
            frame_len: h.frame_len as usize,
            groups: h.groups as usize,
            g: BTreeMap::new(),
            used_parity: false,
        });
        if p.groups != h.groups as usize || p.frame_len != h.frame_len as usize {
            return Err(Error::Malformed); // inconsistent with earlier packets of this frame
        }
        let slot = p.g.entry(h.group).or_insert_with(|| (h.k as usize, h.m as usize, vec![None; h.k as usize + h.m as usize]));
        if slot.0 != h.k as usize || slot.1 != h.m as usize {
            return Err(Error::Malformed);
        }
        slot.2[h.idx as usize] = Some(shard);

        let done = p.g.len() == p.groups && p.g.values().all(|(k, _, s)| s.iter().flatten().count() >= *k);
        if done {
            let p = self.pending.remove(&h.frame).expect("present");
            let mut data = Vec::with_capacity(p.frame_len);
            let mut used_parity = p.used_parity;
            for (_, (k, m, mut shards)) in p.g {
                if shards[..k].iter().any(|s| s.is_none()) {
                    used_parity = true;
                    ReedSolomon::new(k, m).map_err(|_| Error::Malformed)?.reconstruct(&mut shards).map_err(|_| Error::Malformed)?;
                }
                for s in shards.into_iter().take(k) {
                    data.extend(s.expect("reconstructed"));
                }
            }
            data.truncate(p.frame_len);
            // Anything older that is still incomplete will never be shown: count it as lost.
            let stale: Vec<u32> = self.pending.range(..h.frame).map(|(f, _)| *f).collect();
            for f in stale {
                self.pending.remove(&f);
                self.stats.frames_lost += 1;
            }
            self.last_done = Some(h.frame);
            self.stats.frames_ok += 1;
            self.stats.frames_fec_repaired += used_parity as u64;
            return Ok(Some(Frame { frame_no: h.frame, keyframe: p.keyframe, data }));
        }
        while self.pending.len() > MAX_PENDING_FRAMES {
            let oldest = *self.pending.keys().next().expect("non-empty");
            self.pending.remove(&oldest);
            self.stats.frames_lost += 1;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{seq::SliceRandom, Rng, SeedableRng};

    const KEY: SessionKey = [42; 16];

    fn frame(len: usize, seed: u64) -> Vec<u8> {
        let mut r = rand::rngs::StdRng::seed_from_u64(seed);
        (0..len).map(|_| r.gen()).collect()
    }

    fn feed(rx: &mut Receiver, pkts: &[Vec<u8>]) -> Option<Frame> {
        pkts.iter().filter_map(|p| rx.push(p)).last()
    }

    #[test]
    fn roundtrip_without_loss_various_sizes() {
        for len in [0, 1, 1099, 1100, 1101, 50_000, 220_000, 400_000] {
            let data = frame(len, len as u64);
            let pkts = Sender::new(&KEY, 7, 20).packetize(5, true, &data).unwrap();
            let mut rx = Receiver::new(&KEY, 7);
            let f = feed(&mut rx, &pkts).unwrap_or_else(|| panic!("len {len}"));
            assert_eq!((f.frame_no, f.keyframe, f.data), (5, true, data), "len {len}");
        }
    }

    #[test]
    fn packets_fit_under_ipv6_min_mtu() {
        let pkts = Sender::new(&KEY, 1, 20).packetize(1, false, &frame(100_000, 1)).unwrap();
        assert!(pkts.iter().all(|p| p.len() + 48 <= 1280)); // + IPv6 and UDP headers
    }

    #[test]
    fn recovers_from_loss_within_parity() {
        let mut r = rand::rngs::StdRng::seed_from_u64(9);
        for trial in 0..50 {
            let data = frame(60_000, trial);
            let mut pkts = Sender::new(&KEY, 7, 25).packetize(1, false, &data).unwrap();
            // 55 data shards + 14 parity: drop 10 random packets.
            pkts.shuffle(&mut r);
            pkts.truncate(pkts.len() - 10);
            let mut rx = Receiver::new(&KEY, 7);
            let f = feed(&mut rx, &pkts).expect("recoverable");
            assert_eq!(f.data, data);
            assert_eq!(rx.stats.frames_fec_repaired, 1);
        }
    }

    #[test]
    fn loss_beyond_parity_gives_no_frame_and_counts_lost() {
        let s = Sender::new(&KEY, 7, 10);
        let mut rx = Receiver::new(&KEY, 7);
        let mut broken = s.packetize(1, false, &frame(30_000, 1)).unwrap();
        broken.truncate(broken.len() / 2);
        assert!(feed(&mut rx, &broken).is_none());
        let next = s.packetize(2, false, &frame(1_000, 2)).unwrap();
        assert!(feed(&mut rx, &next).is_some());
        assert_eq!(rx.stats.frames_lost, 1);
    }

    #[test]
    fn wrong_key_tampering_and_foreign_streams_are_rejected() {
        let pkts = Sender::new(&KEY, 7, 20).packetize(1, false, &frame(5_000, 3)).unwrap();
        let mut bad_key = Receiver::new(&[1; 16], 7);
        assert!(feed(&mut bad_key, &pkts).is_none());
        assert_eq!(bad_key.stats.rejected as usize, pkts.len());

        let mut tampered = pkts.clone();
        let last = tampered[0].len() - 1;
        tampered[0][last] ^= 1; // payload
        tampered[1][12] ^= 1; // authenticated header field (frame_len)
        let mut rx = Receiver::new(&KEY, 7);
        feed(&mut rx, &tampered);
        assert_eq!(rx.stats.rejected, 2);

        let mut other = Receiver::new(&KEY, 8);
        assert!(feed(&mut other, &pkts).is_none());
        assert!(!pkts.iter().any(|p| Receiver::new(&KEY, 7).push(&p[..10]).is_some()));
    }

    #[test]
    fn out_of_order_and_duplicate_packets_are_fine() {
        let data = frame(20_000, 4);
        let mut pkts = Sender::new(&KEY, 7, 20).packetize(1, false, &data).unwrap();
        let dup = pkts[3].clone();
        pkts.push(dup);
        pkts.reverse();
        let mut rx = Receiver::new(&KEY, 7);
        assert_eq!(feed(&mut rx, &pkts).unwrap().data, data);
    }
}
