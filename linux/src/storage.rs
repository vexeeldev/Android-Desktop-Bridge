use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

#[derive(Clone, Debug, PartialEq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub name: String,
    pub public_key: String,
    pub paired_at: u64,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trusted_devices (
                device_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                public_key TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;

        info!("SQLite database initialized at {:?}", db_path);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trusted_devices (
                device_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                public_key TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn save_trusted_device(&self, dev: &TrustedDevice) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO trusted_devices (device_id, name, public_key, paired_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(device_id) DO UPDATE SET
                name = excluded.name,
                public_key = excluded.public_key,
                last_seen = excluded.last_seen",
            params![
                dev.device_id,
                dev.name,
                dev.public_key,
                dev.paired_at,
                dev.last_seen
            ],
        )?;
        info!("Saved trusted device: {} ({})", dev.name, dev.device_id);
        Ok(())
    }

    pub fn get_trusted_device(&self, device_id: &str) -> Result<Option<TrustedDevice>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_id, name, public_key, paired_at, last_seen
             FROM trusted_devices WHERE device_id = ?1",
        )?;

        let mut rows = stmt.query(params![device_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(TrustedDevice {
                device_id: row.get(0)?,
                name: row.get(1)?,
                public_key: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_trusted_devices(&self) -> Result<Vec<TrustedDevice>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_id, name, public_key, paired_at, last_seen
             FROM trusted_devices ORDER BY last_seen DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TrustedDevice {
                device_id: row.get(0)?,
                name: row.get(1)?,
                public_key: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?;

        let mut list = Vec::new();
        for dev in rows {
            list.push(dev?);
        }
        Ok(list)
    }

    #[allow(dead_code)]
    pub fn remove_trusted_device(&self, device_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM trusted_devices WHERE device_id = ?1",
            params![device_id],
        )?;
        Ok(affected > 0)
    }

    #[allow(dead_code)]
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trusted_device_crud() {
        let db = Database::open_in_memory().expect("In-memory DB creation failed");
        let dev = TrustedDevice {
            device_id: "dev_phone_1".into(),
            name: "Pixel 8 Pro".into(),
            public_key: "pubkey_hex_888".into(),
            paired_at: 1000,
            last_seen: 1000,
        };

        db.save_trusted_device(&dev).unwrap();

        let fetched = db.get_trusted_device("dev_phone_1").unwrap();
        assert_eq!(fetched, Some(dev.clone()));

        let list = db.list_trusted_devices().unwrap();
        assert_eq!(list.len(), 1);

        assert!(db.remove_trusted_device("dev_phone_1").unwrap());
        assert_eq!(db.list_trusted_devices().unwrap().len(), 0);
    }

    #[test]
    fn test_settings() {
        let db = Database::open_in_memory().unwrap();
        db.set_setting("relay_url", "ws://127.0.0.1:8080/ws")
            .unwrap();
        assert_eq!(
            db.get_setting("relay_url").unwrap().as_deref(),
            Some("ws://127.0.0.1:8080/ws")
        );
    }
}
