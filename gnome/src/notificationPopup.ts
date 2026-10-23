import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { ActivityEvent } from './types.js';
import { getIconForType, getColorClassForApp, createActivityIcon } from './activityItem.js';

export class NotificationPopupManager {
  private _currentPopup: St.Widget | null = null;
  private _autoHideTimerId: number = 0;
  private _onOpenHubCallback: ((event: ActivityEvent) => void) | null = null;

  constructor(onOpenHub: (event: ActivityEvent) => void) {
    this._onOpenHubCallback = onOpenHub;
  }

  public showNotification(event: ActivityEvent) {
    this.dismissCurrent();

    const monitor = Main.layoutManager.primaryMonitor;
    const panelHeight = Main.panel ? (Main.panel.height || 36) : 36;

    // Outer container
    const popup = new St.Button({
      style_class: 'bridge-notification-popup',
      reactive: true,
      can_focus: true,
      track_hover: true,
      opacity: 0,
    });

    const contentBox = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-notification-box',
    });

    // 1. Header row: App Icon + App Name + Time + Dismiss (X)
    const headerRow = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-notification-header',
      y_align: Clutter.ActorAlign.CENTER,
    });

    const iconWrapper = new St.BoxLayout({
      style_class: `bridge-notification-icon-wrapper ${getColorClassForApp(event.app)}`,
      y_align: Clutter.ActorAlign.CENTER,
    });
    const icon = createActivityIcon(event.app, event.type, 16, event.iconName);
    iconWrapper.add_child(icon);
    headerRow.add_child(iconWrapper);

    const appLabel = new St.Label({
      text: event.app,
      style_class: 'bridge-notification-app-name',
      y_align: Clutter.ActorAlign.CENTER,
    });
    headerRow.add_child(appLabel);

    const timeLabel = new St.Label({
      text: event.timestamp || 'Sekarang',
      style_class: 'bridge-notification-time',
      y_align: Clutter.ActorAlign.CENTER,
      x_expand: true,
    });
    headerRow.add_child(timeLabel);

    const closeBtn = new St.Button({
      style_class: 'bridge-notification-close-btn',
      reactive: true,
      can_focus: true,
    });
    const closeIcon = new St.Icon({
      icon_name: 'window-close-symbolic',
      icon_size: 14,
    });
    closeBtn.set_child(closeIcon);
    closeBtn.connect('clicked', () => {
      this.dismissCurrent();
      return Clutter.EVENT_STOP;
    });
    headerRow.add_child(closeBtn);

    contentBox.add_child(headerRow);

    // 2. Body row: Title and Body message
    const bodyBox = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-notification-body-box',
    });

    const titleLabel = new St.Label({
      text: event.title,
      style_class: 'bridge-notification-title',
    });
    bodyBox.add_child(titleLabel);

    const messageLabel = new St.Label({
      text: event.body,
      style_class: 'bridge-notification-message',
    });
    bodyBox.add_child(messageLabel);

    contentBox.add_child(bodyBox);
    popup.set_child(contentBox);

    // Click handler to open Activity Hub
    popup.connect('clicked', () => {
      this.dismissCurrent();
      if (this._onOpenHubCallback) {
        this._onOpenHubCallback(event);
      }
    });

    // Add to UI group
    Main.layoutManager.uiGroup.add_child(popup);
    this._currentPopup = popup;

    // Calculate position: centered under top panel
    const popupWidth = 340;
    const posX = monitor.x + Math.max(0, (monitor.width - popupWidth) / 2);
    const posY = monitor.y + panelHeight;
    popup.set_position(posX, posY);

    // Animate appearance (slide & fade in)
    popup.ease(
      {
        opacity: 255,
      },
      {
        duration: 250,
        mode: Clutter.AnimationMode.EASE_OUT_QUAD,
      }
    );

    // Auto-hide after 5 seconds
    this._autoHideTimerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 5000, () => {
      this.dismissCurrent();
      return GLib.SOURCE_REMOVE;
    });
  }

  public dismissCurrent() {
    if (this._autoHideTimerId) {
      GLib.source_remove(this._autoHideTimerId);
      this._autoHideTimerId = 0;
    }

    if (this._currentPopup) {
      const popup = this._currentPopup;
      this._currentPopup = null;

      popup.ease(
        {
          opacity: 0,
        },
        {
          duration: 200,
          mode: Clutter.AnimationMode.EASE_OUT_QUAD,
          onComplete: () => {
            if (popup.get_parent()) {
              Main.layoutManager.uiGroup.remove_child(popup);
            }
            popup.destroy();
          },
        }
      );
    }
  }

  public destroy() {
    this.dismissCurrent();
    this._onOpenHubCallback = null;
  }
}
