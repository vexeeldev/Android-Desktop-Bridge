# Android Desktop Bridge

> Seamlessly control and stream your Android phone from a Linux desktop application over the internet or LAN — **without ADB runtime dependencies and without root**.

---

## Repository Structure

```
├── android/            # Android Agent Application (Kotlin + Android SDK)
│   ├── app/            # Mobile Application Module (Foreground Service, KeyStore, OkHttp)
│   ├── build.gradle.kts
│   └── settings.gradle.kts
├── linux/              # Linux Desktop Controller (Rust, GTK4, libadwaita styling, Tokio, SQLite)
│   ├── src/            # Controller client, identity manager, sqlite storage, and GTK4 UI
│   ├── tests/          # End-to-end integration tests
│   └── Cargo.toml
├── relay/              # Standalone Relay & Signaling Server (Rust, Tokio, WebSockets)
│   ├── src/            # Authenticated session registry & message router
│   └── Cargo.toml
├── protocol/           # Shared Protocol & Cryptographic Specification (Rust)
│   ├── src/            # Envelope models, Ed25519 crypto, deterministic Device IDs
│   └── Cargo.toml
├── tools/
│   └── mock_agent/     # Mock Android Agent CLI for automated protocol & pairing testing
├── docs/
│   ├── ARCHITECTURE.md # Architectural specification and security model
│   └── PROTOCOL.md     # Protocol v1 format and message reference
└── Cargo.toml          # Monorepo Cargo Workspace configuration
```

---

## Architecture Overview

```
 [ Android Phone ] (Kotlin)
       │  - Ed25519 identity in KeyStore
       │  - Background Foreground Service with Partial WakeLock
       ▼
 [ WebSocket / WebRTC ] (Authenticated Handshake & Signaling)
       ▼
 [ Rust Relay Server ] (Tokio)
       ▲
 [ WebSocket / WebRTC ]
       │
 [ Linux Desktop ] (Rust + GTK4 + libadwaita + SQLite)
       • Status Badge: "● Android Connected"
       • Mouse -> Touch input, Keyboard -> Android, Screen Streaming
```

---

## Quick Start & Execution

### 1. Run the Relay Server

```bash
# Bind to 0.0.0.0:8080 or custom port
RELAY_BIND="0.0.0.0:8080" cargo run -p bridge-relay
```

### 2. Run the Linux Desktop Controller

```bash
# Launch GTK4 GUI Controller
cargo run -p bridge-linux -- --relay ws://127.0.0.1:8080

# Or run in Headless CLI mode:
cargo run -p bridge-linux -- --headless --relay ws://127.0.0.1:8080
```

### 3. Run the Android Agent

#### Build Debug APK:
```bash
cd android
./gradlew assembleDebug
# Generated APK: android/app/build/outputs/apk/debug/app-debug.apk
```

#### Install on connected device or emulator:
```bash
adb install android/app/build/outputs/apk/debug/app-debug.apk
```

1. Open the **Android Desktop Bridge** app.
2. Enter the Relay Server URL (e.g., `ws://10.0.2.2:8080` for emulator or LAN IP).
3. Tap **Start Bridge Service** (starts Foreground Service).
4. Enter the displayed 6-digit PIN into the Linux Controller pairing box and click **Pair with Android**.
5. Once paired, the Linux window and Android app update status to:
   **`● Android Connected`**

### 4. Running the Mock Agent (Automated Testing)

If you don't have an Android device at hand, test the full pairing flow using the Mock Agent CLI:

```bash
cargo run -p bridge-mock-agent -- --relay ws://127.0.0.1:8080 --pin 123456
```

---

## Running Test Suites

### Rust Unit & End-to-End Tests
```bash
cargo test --all
```

Tests include:
- Protocol envelope serialization and deserialization
- Ed25519 signature creation and verification
- Deterministic Device ID computation
- SQLite trusted device persistence
- Full Relay ↔ Linux ↔ Agent pairing integration test

### Android Unit Tests
```bash
cd android
./gradlew testDebugUnitTest
```

Tests include:
- BouncyCastle Ed25519 key generation, derivation, and verification
- Protocol message round-trip encoding and decoding
