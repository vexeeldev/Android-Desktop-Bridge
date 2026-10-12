use bridge_protocol::crypto::DeviceKeyPair;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tracing::info;

pub struct IdentityManager {
    keypair: DeviceKeyPair,
    config_dir: PathBuf,
}

impl IdentityManager {
    pub fn load_or_create(custom_dir: Option<&Path>) -> io::Result<Self> {
        let config_dir = match custom_dir {
            Some(d) => d.to_path_buf(),
            None => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config").join("android-desktop-bridge")
            }
        };

        fs::create_dir_all(&config_dir)?;

        let key_path = config_dir.join("identity.key");
        let keypair = if key_path.exists() {
            let seed_bytes = fs::read(&key_path)?;
            if seed_bytes.len() != 32 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid identity key length",
                ));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&seed_bytes);
            info!("Loaded existing device identity from {:?}", key_path);
            DeviceKeyPair::from_seed(&seed)
        } else {
            let new_kp = DeviceKeyPair::generate();
            let seed = new_kp.to_seed();

            // Write with strict 0600 permissions
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&key_path)?;

            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&key_path, perms)?;

            file.write_all(&seed)?;
            info!(
                "Generated new device identity ({}) and saved with 0600 permissions to {:?}",
                new_kp.device_id(),
                key_path
            );
            new_kp
        };

        Ok(Self {
            keypair,
            config_dir,
        })
    }

    pub fn keypair(&self) -> &DeviceKeyPair {
        &self.keypair
    }

    pub fn device_id(&self) -> String {
        self.keypair.device_id()
    }

    pub fn public_key_hex(&self) -> String {
        self.keypair.public_key_hex()
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("bridge_id_test_{}", rand::random::<u32>()));
        let id_mgr1 = IdentityManager::load_or_create(Some(&temp_dir)).expect("Creation failed");
        let dev_id1 = id_mgr1.device_id();

        // Load again from same directory
        let id_mgr2 = IdentityManager::load_or_create(Some(&temp_dir)).expect("Reload failed");
        assert_eq!(dev_id1, id_mgr2.device_id());
        assert_eq!(id_mgr1.public_key_hex(), id_mgr2.public_key_hex());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
