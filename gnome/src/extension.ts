import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import GLib from 'gi://GLib';
import { ActivityStore, MockActivityProvider } from './activityHub.js';
import { BridgeDaemonActivityProvider } from './daemonProvider.js';
import { BridgeIndicator, BridgeIndicatorType } from './indicator.js';
import { NotificationPopupManager } from './notificationPopup.js';
import { setExtensionPath } from './activityItem.js';
import { IActivityProvider } from './types.js';

export default class BridgeGnomeExtension {
  private _context: any;
  private _store: ActivityStore | null = null;
  private _provider: IActivityProvider | null = null;
  private _mockProvider: MockActivityProvider | null = null;
  private _indicator: BridgeIndicatorType | null = null;
  private _notificationManager: NotificationPopupManager | null = null;

  constructor(context: any) {
    this._context = context;
  }

  enable() {
    log('[Bridge GNOME] Enabling extension...');
    setExtensionPath(this._context.path);

    // 1. Initialize Activity Store
    this._store = new ActivityStore();

    // 2. Initialize Custom Notification Popup Manager
    this._notificationManager = new NotificationPopupManager((event) => {
      if (this._indicator) {
        this._indicator.openWithEvent(event);
      }
    });

    // 3. Determine Provider Mode (REAL by default, or MOCK via BRIDGE_MODE=MOCK)
    const mode = (GLib.getenv('BRIDGE_MODE') || 'REAL').toUpperCase();
    log(`[Bridge GNOME] Running in mode: ${mode}`);

    this._mockProvider = new MockActivityProvider();

    if (mode === 'MOCK') {
      this._provider = this._mockProvider;
      this._provider.initialize(this._store);
    } else {
      // REAL Mode: Connect to Bridge Linux Daemon via Local IPC
      const daemonProvider = new BridgeDaemonActivityProvider();
      daemonProvider.initialize(this._store, (event) => {
        if (this._notificationManager) {
          this._notificationManager.showNotification(event);
        }
      });
      this._provider = daemonProvider;
    }

    // 4. Initialize Indicator with Hub and Notification Simulation trigger
    const onSimulate = () => {
      if (this._mockProvider && this._store && this._notificationManager) {
        // Allow simulating event for quick manual testing
        const newEvent = this._mockProvider.simulateIncomingEvent();
        this._notificationManager.showNotification(newEvent);
      }
    };

    this._indicator = new BridgeIndicator(
      this._store,
      this._provider,
      onSimulate
    ) as BridgeIndicatorType;

    // 5. Add to GNOME Top Bar Status Area (in center box, directly to the left of the clock)
    Main.panel.addToStatusArea('bridge-gnome-indicator', this._indicator, 0, 'center');

    log('[Bridge GNOME] Extension successfully enabled.');
  }

  disable() {
    log('[Bridge GNOME] Disabling extension...');

    if (this._indicator) {
      this._indicator.destroy();
      this._indicator = null;
    }

    if (this._notificationManager) {
      this._notificationManager.destroy();
      this._notificationManager = null;
    }

    if (this._provider) {
      this._provider.destroy();
      this._provider = null;
    }

    if (this._mockProvider) {
      this._mockProvider.destroy();
      this._mockProvider = null;
    }

    this._store = null;

    log('[Bridge GNOME] Extension successfully disabled.');
  }
}
