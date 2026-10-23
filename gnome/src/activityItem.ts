import St from 'gi://St';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import { ActivityEvent, IActivityStore } from './types.js';

let _extensionPath = '';

export function setExtensionPath(path: string) {
  _extensionPath = path;
}

export function getWhatsAppIconPath(): string {
  const candidates = [
    _extensionPath ? `${_extensionPath}/public/whatsapp.webp` : '',
    `${GLib.get_home_dir()}/.local/share/gnome-shell/extensions/bridge-gnome@bridge.org/public/whatsapp.webp`,
    `${GLib.get_current_dir()}/public/whatsapp.webp`,
  ];
  for (const path of candidates) {
    if (path && GLib.file_test(path, GLib.FileTest.EXISTS)) {
      return path;
    }
  }
  return '';
}

export function createActivityIcon(app: string, type: string, size: number, customIconName?: string): St.Icon {
  const lower = (app || '').toLowerCase();
  if (lower.includes('whatsapp')) {
    const waPath = getWhatsAppIconPath();
    if (waPath) {
      const file = Gio.File.new_for_path(waPath);
      return new St.Icon({
        gicon: Gio.FileIcon.new(file),
        icon_size: Math.round(size * 1.35),
        style_class: 'bridge-item-icon',
      });
    }
  }

  return new St.Icon({
    icon_name: customIconName || getIconForType(type, app),
    icon_size: size,
    style_class: 'bridge-item-icon',
  });
}

export function getIconForType(type: string, app: string): string {
  switch (type) {
    case 'CALL':
      return 'call-start-symbolic';
    case 'MESSAGE':
      if (app.toLowerCase().includes('whatsapp')) return 'chat-symbolic';
      return 'mail-unread-symbolic';
    case 'SCHOOL':
      return 'x-office-document-symbolic';
    case 'EMAIL':
      return 'mail-message-new-symbolic';
    case 'DOWNLOAD':
      return 'emblem-downloads-symbolic';
    case 'BATTERY':
      return 'battery-full-symbolic';
    case 'SYSTEM':
      return 'preferences-system-symbolic';
    default:
      return 'dialog-information-symbolic';
  }
}

export function getColorClassForApp(app: string): string {
  const lower = app.toLowerCase();
  if (lower.includes('whatsapp')) return 'bridge-icon-whatsapp';
  if (lower.includes('phone') || lower.includes('call')) return 'bridge-icon-phone';
  if (lower.includes('school')) return 'bridge-icon-school';
  if (lower.includes('gmail') || lower.includes('email')) return 'bridge-icon-gmail';
  return 'bridge-icon-default';
}

export class ActivityItemWidget {
  public actor: St.Button;
  private _event: ActivityEvent;
  private _store: IActivityStore;
  private _onSelect: (event: ActivityEvent) => void;
  private _badge: St.Label | null = null;
  private _container: St.BoxLayout;

  constructor(
    event: ActivityEvent,
    store: IActivityStore,
    onSelect: (event: ActivityEvent) => void,
    isSelected: boolean = false
  ) {
    this._event = event;
    this._store = store;
    this._onSelect = onSelect;

    this.actor = new St.Button({
      style_class: `bridge-activity-item ${isSelected ? 'bridge-activity-item-active' : ''}`,
      reactive: true,
      can_focus: true,
      track_hover: true,
      x_expand: true,
    });

    this._container = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-activity-item-box',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });

    this._buildUI();
    this.actor.set_child(this._container);

    this.actor.connect('clicked', () => {
      this._onSelect(this._event);
    });
  }

  private _buildUI() {
    // 1. Icon Container
    const iconWrapper = new St.BoxLayout({
      style_class: `bridge-item-icon-wrapper ${getColorClassForApp(this._event.app)}`,
      y_align: Clutter.ActorAlign.CENTER,
    });

    const icon = createActivityIcon(
      this._event.app,
      this._event.type,
      20,
      this._event.iconName
    );
    iconWrapper.add_child(icon);
    this._container.add_child(iconWrapper);

    // 2. Text Column (Title, Body)
    const textColumn = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-item-text-column',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });

    const titleRow = new St.BoxLayout({
      vertical: false,
      x_expand: true,
    });

    const titleLabel = new St.Label({
      text: this._event.title || this._event.app,
      style_class: 'bridge-item-title',
      x_expand: true,
    });
    titleRow.add_child(titleLabel);
    textColumn.add_child(titleRow);

    const bodyLabel = new St.Label({
      text: this._event.body,
      style_class: 'bridge-item-body',
      x_expand: true,
    });
    textColumn.add_child(bodyLabel);

    this._container.add_child(textColumn);

    // 3. Right Status Indicator / Badge (green exclamation dot like reference image)
    if (this._event.unread) {
      this._badge = new St.Label({
        text: '!',
        style_class: 'bridge-item-unread-badge',
        y_align: Clutter.ActorAlign.CENTER,
      });
      this._container.add_child(this._badge);
    }
  }

  public setSelected(selected: boolean) {
    if (selected) {
      this.actor.style_class = 'bridge-activity-item bridge-activity-item-active';
    } else {
      this.actor.style_class = 'bridge-activity-item';
    }
  }

  public getEventId(): string {
    return this._event.id;
  }
}
