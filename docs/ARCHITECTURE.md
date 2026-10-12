# Android Desktop Bridge — Architecture Specification

## 1. System Overview

Android Desktop Bridge connects an Android device to a Linux desktop application over the internet or local area network (LAN) without requiring Android Debug Bridge (ADB) or root privileges.

Communication is brokered via a lightweight Rust Relay/Signaling server over WebSockets and WebRTC channels. Every device possesses a cryptographic identity (Ed25519 keypair) and establishes trust through mutual verification.

```
+--------------------------+                   +--------------------------+
|      Android Agent       |                   |  Linux Desktop Controller|
|         (Kotlin)         |                   |      (Rust + GTK4)       |
|                          |                   |                          |
|  - KeyStore / Ed25519    |                   |  - Ed25519 Identity      |
|  - Foreground Service    |                   |  - SQLite (trusted devs) |
|  - MediaProjection (P5)  |                   |  - Libadwaita / GTK4 UI  |
|  - Accessibility (P6)    |                   |  - Tokio Async Runtime   |
+------------+-------------+                   +------------+-------------+
             |                                              |
             | Authenticated WS Handshake                   | Authenticated WS Handshake
             |                                              |
             +-----------------------+----------------------+
                                     |
                                     v
                       +---------------------------+
                       |        Rust Relay         |
                       |      (Tokio + WS)         |
                       |                           |
                       | - Authenticated Routing   |
                       | - Session Registry        |
                       | - Ephemeral Pairing Room  |
                       | - WebRTC Signaling (P5)   |
                       +---------------------------+
```

---

## 2. Security Model & Device Identity

1. **Cryptographic Identity**:
   - Each device generates an **Ed25519** keypair (32-byte secret seed, 32-byte public key).
   - **Device ID**: Computed deterministically as `dev_` + `SHA256(public_key)[0..16]` (hex encoded).
   - **Key Storage**:
     - **Android**: Persisted via `EncryptedSharedPreferences` backed by the hardware Android KeyStore.
     - **Linux**: Persisted in `~/.config/android-desktop-bridge/identity.key` with strict `0600` POSIX file permissions.

2. **Connection Authentication (Challenge-Response Handshake)**:
   - When connecting to the Relay:
     1. Client sends `HandshakeRequest`:
        $$\text{Challenge} = \text{device\_id} : \text{timestamp} : \text{nonce}$$
        $$\text{Signature} = \text{Sign}_{\text{privkey}}(\text{Challenge})$$
     2. Relay checks that $\text{device\_id} == \text{derive}(\text{public\_key})$.
     3. Relay verifies $\text{Signature}$ with $\text{public\_key}$.
     4. Only authenticated sockets are registered into the session routing registry.

3. **Pairing Flow**:
   - Linux initiates pairing by transmitting a `PairingRequest` containing its public key and a 6-digit PIN entered by the user.
   - Android checks the PIN against its securely generated code.
   - Upon acceptance, Android returns `PairingResponse` with its public key and registers Linux as a trusted controller.
   - Linux saves Android's identity to `trusted_devices` in SQLite and transitions to **Android Connected**.

---

## 3. Component Architecture

### 3.1 Android Agent (`/android`)
- **`DeviceIdentity.kt`**: Ed25519 key generation, device ID derivation, and digital signing via BouncyCastle.
- **`KeyStoreManager.kt`**: Manages secure storage of private seeds and trusted controllers.
- **`BridgeForegroundService.kt`**: Runs as an Android Foreground Service with a `PARTIAL_WAKE_LOCK`. This guarantees that the agent remains connected and controllable even when the screen is turned off or in sleep mode.
- **`RelayClient.kt`**: OkHttp-based WebSocket transport with automated reconnection, ping/pong heartbeats, and message dispatch.
- **`MainActivity.kt`**: Material 3 user interface displaying connection badges, device credentials, and PIN regeneration.

### 3.2 Linux Controller (`/linux`)
- **`identity.rs`**: Local keypair loader and deterministic Device ID manager.
- **`storage.rs`**: SQLite local database managing trusted device records and controller settings.
- **`client.rs`**: Asynchronous Tokio WebSocket client handling authentication, state machine transitions, and relay communication.
- **`ui.rs`**: GTK4 desktop interface styled with Libadwaita aesthetics, dark-mode support, dynamic badges (`● Android Connected`), and pairing cards.
- **Headless mode**: Flag `--headless` for CLI operations, automated CI/CD validation, and server environments.

### 3.3 Relay Server (`/relay`)
- **`session.rs`**: Thread-safe `SessionRegistry` tracking connected controllers and agents.
- **`handler.rs`**: Verifies cryptographic handshakes, manages ping/pong keepalives, and routes addressed packets between peers.

### 3.4 Shared Protocol (`/protocol`)
- Versioned Rust crate providing serialization models, cryptographic primitives, and message validation.
