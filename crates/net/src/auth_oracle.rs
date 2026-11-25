//! Keyless student runtime support. The Room Key may live in another (SYSTEM) process: the student only ever
//! asks "is this the tutor's proof?" and "give me my own proof", so someone who can read the student process
//! still cannot learn the key or forge a tutor proof.
use crate::RoomKey;

pub trait StudentAuth: Send + Sync {
    fn verify_tutor(&self, exporter: &[u8; 32], nonce: &[u8; 16], tutor_nonce: &[u8; 16], proof: &[u8; 32]) -> bool;
    fn prove_student(&self, exporter: &[u8; 32], tutor_nonce: &[u8; 16], nonce: &[u8; 16]) -> [u8; 32];
}

impl StudentAuth for RoomKey {
    fn verify_tutor(&self, ex: &[u8; 32], nonce: &[u8; 16], tn: &[u8; 16], p: &[u8; 32]) -> bool {
        use subtle::ConstantTimeEq;
        bool::from(crate::proof(self, b'T', ex, nonce, tn).ct_eq(p))
    }
    fn prove_student(&self, ex: &[u8; 32], tn: &[u8; 16], nonce: &[u8; 16]) -> [u8; 32] {
        crate::proof(self, b'S', ex, tn, nonce)
    }
}

pub fn request_verify(ex: &[u8; 32], nonce: &[u8; 16], tn: &[u8; 16], p: &[u8; 32]) -> Vec<u8> {
    [b"V".as_slice(), ex, nonce, tn, p].concat()
}

pub fn request_prove(ex: &[u8; 32], tn: &[u8; 16], nonce: &[u8; 16]) -> Vec<u8> {
    [b"P".as_slice(), ex, tn, nonce].concat()
}

pub const PING: &[u8] = b"H";

/// Key-holder side. Malformed or unknown requests get an empty answer; only the student role can be proved.
pub fn answer(auth: &dyn StudentAuth, req: &[u8]) -> Vec<u8> {
    let a32 = |s: &[u8]| <[u8; 32]>::try_from(s).ok();
    let a16 = |s: &[u8]| <[u8; 16]>::try_from(s).ok();
    match req.first() {
        Some(b'H') if req.len() == 1 => b"K".to_vec(),
        Some(b'V') if req.len() == 97 => match (a32(&req[1..33]), a16(&req[33..49]), a16(&req[49..65]), a32(&req[65..97])) {
            (Some(ex), Some(n), Some(tn), Some(p)) => vec![auth.verify_tutor(&ex, &n, &tn, &p) as u8],
            _ => vec![],
        },
        Some(b'P') if req.len() == 65 => match (a32(&req[1..33]), a16(&req[33..49]), a16(&req[49..65])) {
            (Some(ex), Some(tn), Some(n)) => auth.prove_student(&ex, &tn, &n).to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Student side over any transport: `call` sends the request and returns the answer.
pub struct Remote<F: Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync>(pub F);

impl<F: Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync> StudentAuth for Remote<F> {
    fn verify_tutor(&self, ex: &[u8; 32], n: &[u8; 16], tn: &[u8; 16], p: &[u8; 32]) -> bool {
        (self.0)(&request_verify(ex, n, tn, p)).is_some_and(|a| a == [1])
    }
    fn prove_student(&self, ex: &[u8; 32], tn: &[u8; 16], n: &[u8; 16]) -> [u8; 32] {
        // a failed call gives an all-zero proof, which the tutor rejects
        (self.0)(&request_prove(ex, tn, n)).and_then(|a| <[u8; 32]>::try_from(a).ok()).unwrap_or([0; 32])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: RoomKey = [7; 32];

    #[test]
    fn garbage_gets_no_answer_and_ping_works() {
        assert_eq!(answer(&K, PING), b"K");
        for bad in [&b""[..], b"X", b"V", b"P1234", &[b'V'; 96], &[b'P'; 66]] {
            assert!(answer(&K, bad).is_empty(), "{bad:?}");
        }
    }

    #[test]
    fn the_holder_proves_only_the_student_role_and_verifies_the_tutor_one() {
        let (ex, a, b) = ([1u8; 32], [2u8; 16], [3u8; 16]);
        let prove = answer(&K, &request_prove(&ex, &a, &b));
        assert_eq!(prove.len(), 32);
        // proving "as student" must not hand out a value the tutor role would accept for the same data
        let tutor_proof = crate::proof(&K, b'T', &ex, &a, &b);
        assert_ne!(prove, tutor_proof.to_vec());
        assert_eq!(answer(&K, &request_verify(&ex, &a, &b, &tutor_proof)), [1]);
        assert_eq!(answer(&K, &request_verify(&ex, &a, &b, &[0; 32])), [0]);
        assert_eq!(answer(&K, &request_verify(&ex, &a, &b, &<[u8; 32]>::try_from(prove).unwrap())), [0], "a student proof is no tutor proof");
    }

    #[test]
    fn a_failing_transport_never_authenticates() {
        let r = Remote(|_: &[u8]| None);
        assert!(!r.verify_tutor(&[0; 32], &[0; 16], &[0; 16], &[0; 32]));
        assert_eq!(r.prove_student(&[0; 32], &[0; 16], &[0; 16]), [0; 32]);
    }
}
