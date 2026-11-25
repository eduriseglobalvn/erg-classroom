//! QUIC control channel with mutual Room-Key authentication bound to the TLS session.
//! Student = QUIC server, Tutor = QUIC client. Certificates are self-signed; trust comes from
//! the HMAC proof (keyed with the shared Room Key and the TLS exporter), not from the PKI.
pub mod auth_oracle;
pub mod discovery;
pub mod transfer;

use erg_proto::{decode, encode, Command, Event, PROTO_VERSION};
use hmac::{Hmac, Mac};
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub type RoomKey = [u8; 32];
/// Used when the school installs no key of its own, so nobody has to type one. It is public (it is in this source):
/// it keeps other products out, not people. A school that wants a private key puts 64 hex characters in room.key
/// on the tutor and passes ROOMKEY to the student installer. write-config.ps1 derives the same bytes from the same text.
pub const DEFAULT_ROOM_KEY: RoomKey = *b"ERG Classroom default room key 1";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls: {0}")]
    Tls(String),
    #[error("connect: {0}")]
    Connect(String),
    #[error("codec: {0}")]
    Codec(#[from] postcard::Error),
    #[error("authentication failed")]
    AuthFailed,
    #[error("connection closed")]
    Closed,
}

pub struct Identity {
    pub cert: CertificateDer<'static>,
    pub key: PrivatePkcs8KeyDer<'static>,
    pub fingerprint: [u8; 32],
}

pub fn new_identity(name: &str) -> Result<Identity, Error> {
    let ck = rcgen::generate_simple_self_signed(vec![name.to_string()])
        .map_err(|e| Error::Tls(e.to_string()))?;
    let cert = ck.cert.der().clone();
    let fingerprint: [u8; 32] = Sha256::digest(cert.as_ref()).into();
    Ok(Identity {
        cert,
        key: PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()),
        fingerprint,
    })
}

fn proof(key: &RoomKey, role: u8, exporter: &[u8; 32], a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac key");
    m.update(&[role]);
    m.update(exporter);
    m.update(a);
    m.update(b);
    m.finalize().into_bytes().into()
}

fn exporter(c: &Connection) -> Result<[u8; 32], Error> {
    let mut out = [0u8; 32];
    c.export_keying_material(&mut out, b"erg-classroom auth v1", b"")
        .map_err(|_| Error::Tls("exporter".into()))?;
    Ok(out)
}

#[derive(Debug)]
struct AcceptAny;

impl ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        s: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            m,
            c,
            s,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        s: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            m,
            c,
            s,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn student_endpoint(id: &Identity, bind: SocketAddr) -> Result<Endpoint, Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Error::Tls(e.to_string()))?
        .with_no_client_auth()
        .with_single_cert(
            vec![id.cert.clone()],
            PrivateKeyDer::Pkcs8(id.key.clone_key()),
        )
        .map_err(|e| Error::Tls(e.to_string()))?;
    let qs = quinn::crypto::rustls::QuicServerConfig::try_from(tls)
        .map_err(|e| Error::Tls(e.to_string()))?;
    let mut cfg = ServerConfig::with_crypto(Arc::new(qs));
    let t = Arc::get_mut(&mut cfg.transport).expect("fresh transport");
    t.keep_alive_interval(Some(std::time::Duration::from_secs(3)));
    t.max_concurrent_uni_streams(8u32.into()); // bounds simultaneous incoming file transfers
    t.max_concurrent_bidi_streams(4u32.into());
    t.max_idle_timeout(Some(
        std::time::Duration::from_secs(12).try_into().expect("timeout"),
    ));
    Ok(Endpoint::server(cfg, bind)?)
}

pub fn tutor_endpoint() -> Result<Endpoint, Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Error::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAny))
        .with_no_client_auth();
    let qc = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|e| Error::Tls(e.to_string()))?;
    let mut ep = Endpoint::client("0.0.0.0:0".parse().expect("addr"))?;
    ep.set_default_client_config(ClientConfig::new(Arc::new(qc)));
    Ok(ep)
}

/// Framed bidirectional message stream.
pub struct Channel<S, R> {
    tx: SendStream,
    rx: RecvStream,
    buf: Vec<u8>,
    _p: std::marker::PhantomData<(S, R)>,
}

impl<S: serde::Serialize, R: for<'a> serde::Deserialize<'a>> Channel<S, R> {
    fn new(tx: SendStream, rx: RecvStream) -> Self {
        Self { tx, rx, buf: Vec::new(), _p: Default::default() }
    }
    pub async fn send(&mut self, m: &S) -> Result<(), Error> {
        self.tx.write_all(&encode(m)?).await.map_err(|_| Error::Closed)
    }
    pub async fn recv(&mut self) -> Result<R, Error> {
        loop {
            match decode::<R>(&self.buf) {
                Ok(Some((m, n))) => {
                    self.buf.drain(..n);
                    return Ok(m);
                }
                Ok(None) => {}
                // a complete frame we cannot read is a message added by a newer build: skip it, keep the connection
                Err(e) => {
                    let n = self.buf.get(..4).map_or(usize::MAX, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
                    if n > erg_proto::MAX_FRAME { return Err(e.into()); }
                    self.buf.drain(..4 + n);
                    continue;
                }
            }
            let mut chunk = [0u8; 16 * 1024];
            match self.rx.read(&mut chunk).await.map_err(|_| Error::Closed)? {
                Some(n) => self.buf.extend_from_slice(&chunk[..n]),
                None => return Err(Error::Closed),
            }
        }
    }
}

pub type StudentChannel = Channel<Event, Command>;
pub type TutorChannel = Channel<Command, Event>;

/// Student side: accept the control stream and verify the tutor knows the Room Key.
pub async fn student_accept(conn: &Connection, key: &RoomKey) -> Result<StudentChannel, Error> {
    student_accept_with(conn, Arc::new(*key)).await
}

/// Same, but the Room Key may be held by another process (see `auth_oracle`).
pub async fn student_accept_with(conn: &Connection, auth: Arc<dyn auth_oracle::StudentAuth>) -> Result<StudentChannel, Error> {
    let (tx, rx) = conn.open_bi().await.map_err(|_| Error::Closed)?;
    let mut ch: StudentChannel = Channel::new(tx, rx);
    let ex = exporter(conn)?;
    let nonce: [u8; 16] = rand::random();
    ch.send(&Event::Challenge { nonce }).await?;
    match ch.recv().await? {
        Command::Auth { tutor_nonce, proof: p } => {
            // the key holder may be another process, so ask off the async threads
            let a = auth.clone();
            let ok = tokio::task::spawn_blocking(move || a.verify_tutor(&ex, &nonce, &tutor_nonce, &p)).await.map_err(|_| Error::AuthFailed)?;
            if !ok {
                return Err(Error::AuthFailed);
            }
            let mine = tokio::task::spawn_blocking(move || auth.prove_student(&ex, &tutor_nonce, &nonce)).await.map_err(|_| Error::AuthFailed)?;
            ch.send(&Event::Ready { proto_ver: PROTO_VERSION, proof: mine }).await?;
            Ok(ch)
        }
        _ => Err(Error::AuthFailed),
    }
}

/// Tutor side: connect, prove knowledge of the Room Key, and verify the student does too.
pub async fn tutor_connect(
    ep: &Endpoint,
    addr: SocketAddr,
    key: &RoomKey,
) -> Result<(Connection, TutorChannel), Error> {
    let conn = ep
        .connect(addr, "erg-student")
        .map_err(|e| Error::Connect(e.to_string()))?
        .await
        .map_err(|e| Error::Connect(e.to_string()))?;
    let (tx, rx) = conn.accept_bi().await.map_err(|_| Error::Closed)?;
    let mut ch: TutorChannel = Channel::new(tx, rx);
    let ex = exporter(&conn)?;
    let Event::Challenge { nonce } = ch.recv().await? else {
        return Err(Error::AuthFailed);
    };
    let tutor_nonce: [u8; 16] = rand::random();
    ch.send(&Command::Auth {
        tutor_nonce,
        proof: proof(key, b'T', &ex, &nonce, &tutor_nonce),
    })
    .await?;
    match ch.recv().await? {
        Event::Ready { proto_ver, proof: p }
            if proto_ver == PROTO_VERSION
                && bool::from(proof(key, b'S', &ex, &tutor_nonce, &nonce).ct_eq(&p)) =>
        {
            Ok((conn, ch))
        }
        _ => Err(Error::AuthFailed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pair(
        student_key: RoomKey,
        tutor_key: RoomKey,
    ) -> (Result<StudentChannel, Error>, Result<TutorChannel, Error>) {
        let id = new_identity("erg-student").unwrap();
        let sep = student_endpoint(&id, "127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = sep.local_addr().unwrap();
        let s = tokio::spawn(async move {
            let conn = sep.accept().await.unwrap().await.unwrap();
            let r = student_accept(&conn, &student_key).await;
            std::mem::forget(conn); // keep the connection alive for the test body
            std::mem::forget(sep);
            r
        });
        let tep = tutor_endpoint().unwrap();
        let t = tutor_connect(&tep, addr, &tutor_key).await.map(|(c, ch)| {
            std::mem::forget(c);
            std::mem::forget(tep);
            ch
        });
        (s.await.unwrap(), t)
    }

    #[tokio::test]
    async fn good_key_authenticates_and_commands_flow() {
        let k = [7u8; 32];
        let (s, t) = pair(k, k).await;
        let (mut s, mut t) = (s.unwrap(), t.unwrap());
        t.send(&Command::OpenUrl("https://example.com".into())).await.unwrap();
        assert_eq!(s.recv().await.unwrap(), Command::OpenUrl("https://example.com".into()));
        s.send(&Event::HelpRequest("giúp em".into())).await.unwrap();
        assert_eq!(t.recv().await.unwrap(), Event::HelpRequest("giúp em".into()));
    }

    /// A keyless student that asks the key holder authenticates like one with the key; a wrong holder key fails.
    #[tokio::test]
    async fn a_student_without_the_key_authenticates_through_the_key_holder() {
        use auth_oracle::{answer, Remote};
        async fn run(holder_key: RoomKey, tutor_key: RoomKey) -> (Result<StudentChannel, Error>, Result<TutorChannel, Error>) {
            let holder = move |req: &[u8]| Some(answer(&holder_key, req));
            let id = new_identity("erg-student").unwrap();
            let sep = student_endpoint(&id, "127.0.0.1:0".parse().unwrap()).unwrap();
            let addr = sep.local_addr().unwrap();
            let s = tokio::spawn(async move {
                let conn = sep.accept().await.unwrap().await.unwrap();
                let r = student_accept_with(&conn, Arc::new(Remote(holder))).await;
                std::mem::forget(conn);
                std::mem::forget(sep);
                r
            });
            let tep = tutor_endpoint().unwrap();
            let t = tutor_connect(&tep, addr, &tutor_key).await.map(|(c, ch)| {
                std::mem::forget(c);
                std::mem::forget(tep);
                ch
            });
            (s.await.unwrap(), t)
        }
        let k = [7u8; 32];
        let (s, t) = run(k, k).await;
        let (mut s, mut t) = (s.unwrap(), t.unwrap());
        t.send(&Command::Unlock).await.unwrap();
        assert_eq!(s.recv().await.unwrap(), Command::Unlock);
        let (s, t) = run([9u8; 32], k).await;
        assert!(matches!(s, Err(Error::AuthFailed)) && t.is_err());
    }

    #[tokio::test]
    async fn wrong_key_is_rejected_on_both_sides() {
        let (s, t) = pair([1u8; 32], [2u8; 32]).await;
        assert!(matches!(s, Err(Error::AuthFailed)));
        assert!(t.is_err());
    }
}
