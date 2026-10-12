# Android Desktop Bridge — Protocol Specification (v1)

## 1. Protocol Envelope

Every packet transferred over the control channel is wrapped in a versioned `MessageEnvelope`:

```json
{
  "version": 1,
  "id": "1773281920000_4821",
  "timestamp": 1773281920000,
  "sender": "dev_63bcf23a3c4ad17369589c426b83f8c2",
  "recipient": "dev_bcc503de7f001e8f78c2bc0843ac53b4",
  "payload": {
    "type": "<PayloadType>",
    "data": { ... }
  }
}
```

| Field | Type | Description |
|---|---|---|
| `version` | `u32` | Protocol version (currently `1`). |
| `id` | `String` | Unique sequence / message identifier. |
| `timestamp` | `u64` | Milliseconds since Unix epoch. |
| `sender` | `String` | Deterministic Device ID of sender (`dev_<hex16>`). |
| `recipient` | `String?` | Optional destination Device ID (`null` = relay/broadcast). |
| `payload` | `Object` | Typed message payload. |

---

## 2. Core Messages (Phase 1–3)

### 2.1 HandshakeRequest
Client sends this immediately upon establishing a WebSocket connection.

```json
{
  "type": "HandshakeRequest",
  "data": {
    "device_id": "dev_63bcf23a3c4ad17369589c426b83f8c2",
    "public_key": "92e8ce5d99e0fe80e15429cc... (64 hex characters)",
    "timestamp": 1773281920000,
    "nonce": "e4f8b1c201ab",
    "signature": "81f08bc4... (128 hex characters)",
    "role": "controller"
  }
}
```

### 2.2 HandshakeResponse
Relay responds to validate the session.

```json
{
  "type": "HandshakeResponse",
  "data": {
    "success": true,
    "session_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
    "message": "Authentication successful"
  }
}
```

### 2.3 PairingRequest
Linux controller initiates pairing with Android agent.

```json
{
  "type": "PairingRequest",
  "data": {
    "initiator_id": "dev_63bcf23a3c4ad17369589c426b83f8c2",
    "initiator_name": "Linux Desktop Controller",
    "initiator_pubkey": "92e8ce5d99e0fe80e15429cc...",
    "pairing_code": "778899"
  }
}
```

### 2.4 PairingResponse
Android validates the pairing code and responds.

```json
{
  "type": "PairingResponse",
  "data": {
    "accepted": true,
    "responder_id": "dev_bcc503de7f001e8f78c2bc0843ac53b4",
    "responder_name": "Google Pixel 8 Pro",
    "responder_pubkey": "a1b2c3d4...",
    "message": "Pairing accepted by Android"
  }
}
```

### 2.5 StatusUpdate
Notification of connection status changes.

```json
{
  "type": "StatusUpdate",
  "data": {
    "state": "Android Connected",
    "details": "Paired with Linux Controller"
  }
}
```

### 2.6 Ping & Pong
Connection keepalive messages sent every 15 seconds.

```json
{
  "type": "Ping",
  "data": {
    "nonce": 9821405
  }
}
```

```json
{
  "type": "Pong",
  "data": {
    "nonce": 9821405
  }
}
```

---

## 3. Extensible Reserved Messages (Phases 4–10)

| Message Type | Parameters | Target Phase |
|---|---|---|
| `DeviceInfoRequest` | `{}` | Phase 4 |
| `DeviceInfoResponse` | `{ model, android_version, battery_level, is_charging, screen_width, screen_height, orientation }` | Phase 4 |
| `ScreenStart` | `{ width, height, fps }` | Phase 5 (WebRTC signaling) |
| `ScreenStop` | `{}` | Phase 5 |
| `TouchDown` | `{ pointer_id, x, y }` | Phase 6 |
| `TouchMove` | `{ pointer_id, x, y }` | Phase 6 |
| `TouchUp` | `{ pointer_id }` | Phase 6 |
| `KeyEvent` | `{ key_code, pressed, modifiers }` | Phase 6 |
| `ClipboardSet` | `{ text }` | Phase 7 |
| `ClipboardGet` | `{}` | Phase 7 |
| `BatteryStatus` | `{ level, charging }` | Phase 4 |
| `NotificationEvent` | `{ id, app_name, title, text }` | Phase 8 |
| `FileRequest` | `{ file_id, file_name }` | Phase 9 |
| `FileTransfer` | `{ file_id, chunk_index, total_chunks, data_base64, checksum }` | Phase 9 |
| `CallStart` / `CallAnswer` / `CallEnd` | `{ phone_number }` | Phase 10 |
