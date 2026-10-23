import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import {
  ActivityEvent,
  ActivityType,
  DeviceStatus,
  IActivityProvider,
  IActivityStore,
} from './types.js';

interface RawIpcMessage {
  type: string;
  data?: any;
}

/**
 * BridgeDaemonActivityProvider connects to the local Bridge Linux Daemon via Unix domain socket
 * and ingests real Android notifications and device status into the ActivityStore and UI.
 */
export class BridgeDaemonActivityProvider implements IActivityProvider {
  public name = 'BridgeDaemonActivityProvider';
  private _store: IActivityStore | null = null;
  private _onNotification?: (event: ActivityEvent) => void;
  private _cancellable: Gio.Cancellable | null = null;
  private _connection: Gio.SocketConnection | null = null;
  private _dataInputStream: Gio.DataInputStream | null = null;
  private _reconnectTimerId: number = 0;
  private _isDestroyed: boolean = false;
  private _socketPath: string = '';

  constructor(customSocketPath?: string) {
    if (customSocketPath) {
      this._socketPath = customSocketPath;
    } else {
      const runtimeDir = GLib.getenv('XDG_RUNTIME_DIR') || '/tmp';
      this._socketPath = `${runtimeDir}/bridge-daemon.sock`;
    }
  }

  public initialize(store: IActivityStore, onNotification?: (event: ActivityEvent) => void) {
    this._store = store;
    this._onNotification = onNotification;
    this._isDestroyed = false;
    this._connect();
  }

  private _connect() {
    if (this._isDestroyed) return;

    this._cleanupConnection();
    this._cancellable = new Gio.Cancellable();

    const client = new Gio.SocketClient();
    const address = Gio.UnixSocketAddress.new(this._socketPath);

    console.log(`[BridgeDaemonProvider] Connecting to daemon socket at ${this._socketPath}...`);

    client.connect_async(address, this._cancellable, (source, res) => {
      if (this._isDestroyed) return;

      try {
        this._connection = client.connect_finish(res);
        console.log(`[BridgeDaemonProvider] Successfully connected to Bridge Linux daemon!`);

        const inputStream = this._connection.get_input_stream();
        this._dataInputStream = new Gio.DataInputStream({ base_stream: inputStream });

        this._readNextLine();
      } catch (err: any) {
        console.log(`[BridgeDaemonProvider] Connection waiting/retrying: ${err.message}`);
        this._handleDisconnect();
      }
    });
  }

  private _readNextLine() {
    if (this._isDestroyed || !this._dataInputStream || !this._cancellable) return;

    this._dataInputStream.read_line_async(
      GLib.PRIORITY_DEFAULT,
      this._cancellable,
      (source, res) => {
        if (this._isDestroyed) return;

        try {
          const [line, _length] = source.read_line_finish_utf8(res);
          if (line === null) {
            console.log(`[BridgeDaemonProvider] Daemon socket disconnected (EOF)`);
            this._handleDisconnect();
            return;
          }

          const trimmed = line.trim();
          if (trimmed.length > 0) {
            this._handleIpcLine(trimmed);
          }

          // Continue stream read loop
          this._readNextLine();
        } catch (err: any) {
          if (!this._cancellable?.is_cancelled()) {
            console.error(`[BridgeDaemonProvider] Read error: ${err.message}`);
            this._handleDisconnect();
          }
        }
      }
    );
  }

  private _handleIpcLine(line: string) {
    try {
      const msg: RawIpcMessage = JSON.parse(line);
      if (!this._store) return;

      switch (msg.type) {
        case 'DeviceStatus': {
          const data = msg.data || {};
          const status: DeviceStatus = {
            name: data.device_name || (data.connected ? 'Android Connected' : 'Bridge (Disconnected)'),
            connected: !!data.connected,
            deviceId: data.device_id,
            batteryLevel: data.battery_level,
            isCharging: data.is_charging,
          };
          this._store.setDeviceStatus(status);
          console.log(`[BridgeDaemonProvider] Device status: ${status.name} (connected=${status.connected})`);
          break;
        }

        case 'ActivityEvent': {
          const data = msg.data || {};
          const category = this._normalizeCategory(data.category);
          const appName = data.app_name || data.package_name || 'Android';
          const title = data.title || appName;
          const body = data.body || '';
          const timeStr = this._formatTimestamp(data.timestamp);

          const event: ActivityEvent = {
            id: data.event_id || String(Date.now()),
            type: category,
            app: appName,
            title,
            body,
            unread: data.unread !== false,
            timestamp: timeStr,
            iconName: data.icon,
            isOngoing: data.is_ongoing,
            isClearable: data.is_clearable,
            canReply: data.can_reply,
            chatHistory: category === 'MESSAGE' ? [
              {
                id: `msg-${data.event_id || Date.now()}`,
                sender: title,
                text: body,
                time: timeStr,
                isOutgoing: false,
              },
            ] : undefined,
          };

          this._store.addEvent(event);
          console.log(`[BridgeDaemonProvider] Received ActivityEvent: [${event.app}] ${event.title}: ${event.body}`);

          if (this._onNotification) {
            this._onNotification(event);
          }
          break;
        }

        case 'ActivityRemoved': {
          const eventId = msg.data?.event_id;
          if (eventId) {
            this._store.dismissEvent(eventId);
            console.log(`[BridgeDaemonProvider] Removed ActivityEvent: ${eventId}`);
          }
          break;
        }

        default:
          console.log(`[BridgeDaemonProvider] Unknown IPC message type: ${msg.type}`);
          break;
      }
    } catch (e: any) {
      console.error(`[BridgeDaemonProvider] Error parsing IPC JSON: ${e.message}`);
    }
  }

  private _normalizeCategory(cat?: string): ActivityType {
    if (!cat) return 'NOTIFICATION';
    const upper = cat.toUpperCase();
    if (
      upper === 'MESSAGE' ||
      upper === 'CALL' ||
      upper === 'NOTIFICATION' ||
      upper === 'EMAIL' ||
      upper === 'SCHOOL' ||
      upper === 'DOWNLOAD' ||
      upper === 'BATTERY' ||
      upper === 'SYSTEM'
    ) {
      return upper as ActivityType;
    }
    return 'NOTIFICATION';
  }

  private _formatTimestamp(timestampMs?: number): string {
    if (!timestampMs) return 'Sekarang';
    const date = new Date(timestampMs);
    const hours = String(date.getHours()).padStart(2, '0');
    const minutes = String(date.getMinutes()).padStart(2, '0');
    return `${hours}:${minutes}`;
  }

  private _handleDisconnect() {
    this._cleanupConnection();

    if (this._store) {
      this._store.setDeviceStatus({
        name: 'Bridge (Disconnected)',
        connected: false,
      });
    }

    if (!this._isDestroyed && this._reconnectTimerId === 0) {
      this._reconnectTimerId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 3, () => {
        this._reconnectTimerId = 0;
        this._connect();
        return GLib.SOURCE_REMOVE;
      });
    }
  }

  private _cleanupConnection() {
    if (this._cancellable) {
      this._cancellable.cancel();
      this._cancellable = null;
    }
    if (this._dataInputStream) {
      try {
        this._dataInputStream.close(null);
      } catch (_) {}
      this._dataInputStream = null;
    }
    if (this._connection) {
      try {
        this._connection.close(null);
      } catch (_) {}
      this._connection = null;
    }
  }

  public destroy() {
    this._isDestroyed = true;
    if (this._reconnectTimerId !== 0) {
      GLib.source_remove(this._reconnectTimerId);
      this._reconnectTimerId = 0;
    }
    this._cleanupConnection();
    this._store = null;
    this._onNotification = undefined;
    console.log(`[BridgeDaemonProvider] Destroyed`);
  }
}
