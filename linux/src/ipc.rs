use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

pub fn default_ipc_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("bridge-daemon.sock")
    } else {
        PathBuf::from("/tmp/bridge-daemon.sock")
    }
}

pub struct IpcServer {
    socket_path: PathBuf,
    broadcast_tx: broadcast::Sender<String>,
    current_status: Arc<RwLock<Option<String>>>,
}

impl IpcServer {
    pub fn new(custom_path: Option<PathBuf>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(128);
        let socket_path = custom_path.unwrap_or_else(default_ipc_socket_path);

        Self {
            socket_path,
            broadcast_tx,
            current_status: Arc::new(RwLock::new(None)),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn broadcast_line(&self, line: &str) {
        let formatted = if line.ends_with('\n') {
            line.to_string()
        } else {
            format!("{}\n", line)
        };
        let _ = self.broadcast_tx.send(formatted);
    }

    pub async fn update_device_status(
        &self,
        connected: bool,
        device_id: Option<String>,
        device_name: Option<String>,
        battery_level: Option<i32>,
        is_charging: Option<bool>,
    ) {
        let status_obj = serde_json::json!({
            "type": "DeviceStatus",
            "data": {
                "connected": connected,
                "device_id": device_id,
                "device_name": device_name,
                "battery_level": battery_level,
                "is_charging": is_charging
            }
        });

        let line = format!("{}\n", status_obj);
        {
            let mut current = self.current_status.write().await;
            *current = Some(line.clone());
        }
        let _ = self.broadcast_tx.send(line);
    }

    pub fn broadcast_payload(&self, payload: &bridge_protocol::MessagePayload) {
        if let Ok(json) = serde_json::to_string(payload) {
            self.broadcast_line(&json);
        }
    }

    pub async fn run_server(self: Arc<Self>) -> std::io::Result<()> {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        if let Some(parent) = self.socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        info!(
            "Bridge IPC server listening on Unix socket: {:?}",
            self.socket_path
        );

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        server.handle_client(stream).await;
                    });
                }
                Err(e) => {
                    warn!("Error accepting IPC connection: {}", e);
                }
            }
        }
    }

    async fn handle_client(&self, stream: UnixStream) {
        debug!("New IPC client connected (GNOME Extension / CLI)");
        let (reader, mut writer) = stream.into_split();
        let mut rx = self.broadcast_tx.subscribe();

        // Send initial device status if available
        if let Some(status_line) = self.current_status.read().await.clone() {
            if let Err(e) = writer.write_all(status_line.as_bytes()).await {
                debug!("Failed to write initial status to IPC client: {}", e);
                return;
            }
        }

        // Forward broadcast messages to this client
        let send_task = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if writer.write_all(msg.as_bytes()).await.is_err() {
                    break;
                }
            }
        });

        // Read incoming commands from client (for future expansion/ping)
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match buf_reader.read_line(&mut line).await {
                Ok(0) => break, // EOF / disconnected
                Ok(_) => {
                    debug!("Received IPC command from client: {}", line.trim());
                }
                Err(_) => break,
            }
        }

        send_task.abort();
        debug!("IPC client disconnected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_ipc_server_broadcast() {
        let temp_dir = std::env::temp_dir();
        let sock_path = temp_dir.join(format!("test-bridge-ipc-{}.sock", rand::random::<u64>()));

        let server = Arc::new(IpcServer::new(Some(sock_path.clone())));
        let server_clone = Arc::clone(&server);

        tokio::spawn(async move {
            let _ = server_clone.run_server().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let stream = UnixStream::connect(&sock_path)
            .await
            .expect("Failed to connect to IPC");
        let mut reader = BufReader::new(stream);

        // Update status and broadcast
        server
            .update_device_status(
                true,
                Some("dev_1".into()),
                Some("Pixel 8".into()),
                Some(90),
                Some(true),
            )
            .await;

        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .expect("Failed to read status line");
        assert!(line.contains("\"type\":\"DeviceStatus\""));
        assert!(line.contains("\"Pixel 8\""));

        // Broadcast ActivityEvent
        let event = bridge_protocol::MessagePayload::ActivityEvent {
            event_id: "test-event-1".into(),
            category: "MESSAGE".into(),
            package_name: "com.whatsapp".into(),
            app_name: "WhatsApp".into(),
            title: "Budi".into(),
            body: "ke kantor jam berapa?".into(),
            timestamp: 123456789,
            unread: true,
            icon: None,
            conversation_id: None,
            person_name: None,
            is_ongoing: Some(false),
            is_clearable: Some(true),
            can_reply: Some(true),
        };
        server.broadcast_payload(&event);

        line.clear();
        reader
            .read_line(&mut line)
            .await
            .expect("Failed to read activity line");
        assert!(line.contains("\"type\":\"ActivityEvent\""));
        assert!(line.contains("\"event_id\":\"test-event-1\""));
        assert!(line.contains("\"Budi\""));

        let _ = std::fs::remove_file(&sock_path);
    }
}
