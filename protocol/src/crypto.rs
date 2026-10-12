use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::Rng;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Invalid key length or format")]
    InvalidKeyFormat,
    #[error("Invalid signature: {0}")]
    SignatureError(#[from] ed25519_dalek::SignatureError),
    #[error("Verification failed")]
    VerificationFailed,
}

#[derive(Clone)]
pub struct DeviceKeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl DeviceKeyPair {
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn to_seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }

    pub fn device_id(&self) -> String {
        device_id_from_bytes(&self.public_key_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> String {
        let signature: Signature = self.signing_key.sign(message);
        hex::encode(signature.to_bytes())
    }
}

pub fn device_id_from_bytes(pubkey_bytes: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pubkey_bytes);
    let hash = hasher.finalize();
    format!("dev_{}", hex::encode(&hash[..16]))
}

pub fn device_id_from_pubkey_hex(pubkey_hex: &str) -> Result<String, CryptoError> {
    let bytes = hex::decode(pubkey_hex)?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidKeyFormat);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(device_id_from_bytes(&arr))
}

pub fn verify_signature(
    public_key_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<(), CryptoError> {
    let pub_bytes = hex::decode(public_key_hex)?;
    if pub_bytes.len() != 32 {
        return Err(CryptoError::InvalidKeyFormat);
    }
    let mut pub_arr = [0u8; 32];
    pub_arr.copy_from_slice(&pub_bytes);

    let sig_bytes = hex::decode(signature_hex)?;
    if sig_bytes.len() != 64 {
        return Err(CryptoError::InvalidKeyFormat);
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    let verifying_key = VerifyingKey::from_bytes(&pub_arr)?;
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key.verify(message, &signature)?;
    Ok(())
}

pub fn generate_pairing_pin() -> String {
    let mut rng = OsRng;
    let pin: u32 = rng.gen_range(100_000..=999_999);
    pin.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation_and_signing() {
        let kp = DeviceKeyPair::generate();
        let pub_hex = kp.public_key_hex();
        let dev_id = kp.device_id();
        assert!(dev_id.starts_with("dev_"));

        let msg = b"bridge_challenge_nonce_12345";
        let sig = kp.sign(msg);

        assert!(verify_signature(&pub_hex, msg, &sig).is_ok());
        assert!(verify_signature(&pub_hex, b"tampered_message", &sig).is_err());
    }

    #[test]
    fn test_device_id_determinism() {
        let seed = [7u8; 32];
        let kp1 = DeviceKeyPair::from_seed(&seed);
        let kp2 = DeviceKeyPair::from_seed(&seed);
        assert_eq!(kp1.device_id(), kp2.device_id());
        assert_eq!(kp1.public_key_hex(), kp2.public_key_hex());
    }

    #[test]
    fn test_pairing_pin_length() {
        let pin = generate_pairing_pin();
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }
}
