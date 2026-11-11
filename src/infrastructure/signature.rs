//! HMAC-SHA256 signing for the bank settlement webhook.
//!
//! The bank sends `X-Bank-Signature: sha256=<hex>` where `<hex>` is the
//! HMAC-SHA256 of the raw request body keyed with the shared secret.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const HEADER: &str = "x-bank-signature";

pub fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Constant-time verification of a signature header value.
pub fn verify(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(hex_sig) = header.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Ok(sig) = hex::decode(hex_sig) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let sig = sign("s3cret", b"{\"a\":1}");
        assert!(verify("s3cret", b"{\"a\":1}", &sig));
    }

    #[test]
    fn rejects_wrong_secret_body_or_format() {
        let sig = sign("s3cret", b"body");
        assert!(!verify("other", b"body", &sig));
        assert!(!verify("s3cret", b"tampered", &sig));
        assert!(!verify("s3cret", b"body", sig.trim_start_matches("sha256=")));
        assert!(!verify("s3cret", b"body", "sha256=zz"));
    }
}
