use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DeviceSession {
    pub device_id: String,
    pub public_key: String,
    pub role: String,
    pub tx: mpsc::UnboundedSender<String>,
}

#[derive(Clone, Default)]
pub struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<String, DeviceSession>>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, session: DeviceSession) {
        let mut map = self.sessions.write().await;
        info!(
            "Registered device: id={}, role={}, total_online={}",
            session.device_id,
            session.role,
            map.len() + 1
        );
        map.insert(session.device_id.clone(), session);
    }

    pub async fn unregister(&self, device_id: &str) {
        let mut map = self.sessions.write().await;
        if map.remove(device_id).is_some() {
            info!(
                "Unregistered device: id={}, total_online={}",
                device_id,
                map.len()
            );
        }
    }

    pub async fn get_sender(&self, device_id: &str) -> Option<mpsc::UnboundedSender<String>> {
        let map = self.sessions.read().await;
        map.get(device_id).map(|s| s.tx.clone())
    }

    #[allow(dead_code)]
    pub async fn is_online(&self, device_id: &str) -> bool {
        let map = self.sessions.read().await;
        map.contains_key(device_id)
    }

    pub async fn list_devices_by_role(&self, role: &str) -> Vec<DeviceSession> {
        let map = self.sessions.read().await;
        map.values().filter(|s| s.role == role).cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> usize {
        let map = self.sessions.read().await;
        map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_registry() {
        let registry = SessionRegistry::new();
        assert_eq!(registry.count().await, 0);

        let (tx, _rx) = mpsc::unbounded_channel();
        let session = DeviceSession {
            device_id: "dev_test".into(),
            public_key: "pubkey_test".into(),
            role: "agent".into(),
            tx,
        };

        registry.register(session).await;
        assert_eq!(registry.count().await, 1);
        assert!(registry.is_online("dev_test").await);
        assert!(!registry.is_online("dev_other").await);

        let agents = registry.list_devices_by_role("agent").await;
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].device_id, "dev_test");
        assert_eq!(agents[0].public_key, "pubkey_test");

        registry.unregister("dev_test").await;
        assert_eq!(registry.count().await, 0);
        assert!(!registry.is_online("dev_test").await);
    }
}
