//! Licence keys for the Tutor: `ERG1.<payload>.<signature>`, both parts base64url. The payload is the JSON of
//! [`Licence`]; the signature is Ed25519 over the payload text. ERG signs with a private key that is never in this
//! repository; the Tutor checks with the public key in `erg-license.pub`. While that file is empty no key is asked
//! for and the number of students is not limited.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};

/// Students a Tutor without a valid licence can connect.
pub const TRIAL_SEATS: usize = 5;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Licence {
    pub school: String,
    /// Students connected at the same time.
    pub seats: u32,
    /// Last valid moment, seconds since 1970. 0 = never expires.
    pub expires: u64,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    /// Not a key, changed after signing, or signed by someone else.
    Invalid,
    /// A real key whose time is over.
    Expired(Licence),
}

/// The public key built into this program. None = licensing is switched off.
pub fn embedded_key() -> Option<Vec<u8>> {
    B64.decode(include_str!("../erg-license.pub").trim()).ok().filter(|k| k.len() == 32)
}

pub fn verify(text: &str, public: &[u8], now: u64) -> Result<Licence, Error> {
    let text: String = text.split_whitespace().collect(); // pasted from an e-mail: line breaks are not part of the key
    let mut p = text.split('.');
    let (Some("ERG1"), Some(body), Some(sig), None) = (p.next(), p.next(), p.next(), p.next()) else { return Err(Error::Invalid) };
    let sig = B64.decode(sig).map_err(|_| Error::Invalid)?;
    // the signature covers the text exactly as written, so nothing is decoded before it is checked
    UnparsedPublicKey::new(&ED25519, public).verify(body.as_bytes(), &sig).map_err(|_| Error::Invalid)?;
    let l: Licence = B64.decode(body).ok().and_then(|j| serde_json::from_slice(&j).ok()).ok_or(Error::Invalid)?;
    if l.expires != 0 && now > l.expires {
        return Err(Error::Expired(l));
    }
    Ok(l)
}

/// New key pair: (private key as PKCS#8, public key as the text for `erg-license.pub`).
pub fn keygen() -> Result<(Vec<u8>, String), String> {
    let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).map_err(|e| e.to_string())?;
    let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).map_err(|e| e.to_string())?;
    Ok((doc.as_ref().to_vec(), B64.encode(pair.public_key().as_ref())))
}

pub fn sign(l: &Licence, pkcs8: &[u8]) -> Result<String, String> {
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|e| e.to_string())?;
    let body = B64.encode(serde_json::to_vec(l).map_err(|e| e.to_string())?);
    let sig = B64.encode(pair.sign(body.as_bytes()).as_ref());
    Ok(format!("ERG1.{body}.{sig}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_key_is_accepted_and_anything_else_is_not() {
        let (private, public) = keygen().unwrap();
        let public = B64.decode(public).unwrap();
        let l = Licence { school: "THCS Lê Lợi".into(), seats: 40, expires: 1000 };
        let key = sign(&l, &private).unwrap();
        assert_eq!(verify(&key, &public, 999), Ok(l.clone()));
        assert_eq!(verify(&format!("  {}\n{} ", &key[..30], &key[30..]), &public, 1000), Ok(l.clone()), "line breaks from an e-mail");
        assert_eq!(verify(&key, &public, 1001), Err(Error::Expired(l.clone())));

        // more seats, same signature
        let more = B64.encode(serde_json::to_vec(&Licence { seats: 400, ..l.clone() }).unwrap());
        let forged = format!("ERG1.{more}.{}", key.rsplit('.').next().unwrap());
        assert_eq!(verify(&forged, &public, 0), Err(Error::Invalid));
        // signed with another key
        let (other, _) = keygen().unwrap();
        assert_eq!(verify(&sign(&l, &other).unwrap(), &public, 0), Err(Error::Invalid));
        for junk in ["", "ERG1", "ERG1..", "ERG2.a.b", &format!("{key}.x")] {
            assert_eq!(verify(junk, &public, 0), Err(Error::Invalid), "{junk}");
        }
        assert_eq!(verify(&sign(&Licence { expires: 0, ..l.clone() }, &private).unwrap(), &public, u64::MAX).map(|l| l.seats), Ok(40), "0 never expires");
    }
}
