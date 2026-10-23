import GObject from 'gi://GObject';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import { ActivityEvent, IActivityProvider, IActivityStore } from './types.js';
import { ActivityHubWidget } from './activityHub.js';

export const BridgeIndicator = GObject.registerClass(
  {
    GTypeName: 'BridgeIndicator',
  },
  class BridgeIndicator extends PanelMenu.Button {
    private _store!: IActivityStore;
    private _provider!: IActivityProvider;
    private _hubWidget!: ActivityHubWidget;
    private _badgeLabel!: St.Label;
    private _icon!: St.Icon;
    private _unsubscribeStore: (() => void) | null = null;
    private _unsubscribeDeviceStatus: (() => void) | null = null;

    _init(
      store: IActivityStore,
      provider: IActivityProvider,
      onSimulateNotification: () => void
    ) {
      super._init(0.5, 'Bridge Indicator', false);

      this._store = store;
      this._provider = provider;

      // 1. Panel indicator button layout
      const box = new St.BoxLayout({
        style_class: 'bridge-panel-button-box',
        y_align: Clutter.ActorAlign.CENTER,
      });

      this._icon = new St.Icon({
        icon_name: 'smartphone-symbolic',
        style_class: 'system-status-icon bridge-panel-icon',
      });
      box.add_child(this._icon);

      const titleLabel = new St.Label({
        text: 'Bridge',
        style_class: 'bridge-panel-label',
        y_align: Clutter.ActorAlign.CENTER,
      });
      box.add_child(titleLabel);

      this._badgeLabel = new St.Label({
        text: '',
        style_class: 'bridge-panel-badge',
        y_align: Clutter.ActorAlign.CENTER,
        visible: false,
      });
      box.add_child(this._badgeLabel);

      this.add_child(box);

      // 2. Build Activity Hub Widget inside PopupMenu
      this.menu.actor.add_style_class_name('bridge-popup-menu');
      this.menu.actor.style = 'background-color: transparent; border: none; box-shadow: none; margin: 0; padding: 0;';

      this.menu.box.add_style_class_name('bridge-popup-menu-box');
      this.menu.box.style = 'padding: 0; margin: 0; background-color: transparent; border: none; box-shadow: none; border-radius: 0;';

      const menuItem = new PopupMenu.PopupBaseMenuItem({
        can_focus: false,
        activate: false,
        style_class: 'bridge-menu-base-item',
      });
      menuItem.style = 'padding: 0; margin: 0; background-color: transparent; border: none; box-shadow: none; border-radius: 0;';

      this._hubWidget = new ActivityHubWidget(
        this._store,
        this._provider,
        onSimulateNotification
      );
      menuItem.add_child(this._hubWidget.actor);
      this.menu.addMenuItem(menuItem);

      // 3. Listen to store unread changes and device status
      this._updateBadge();
      this._unsubscribeStore = this._store.subscribe(() => {
        this._updateBadge();
      });

      this._unsubscribeDeviceStatus = this._store.subscribeDeviceStatus((status) => {
        if (status.connected) {
          this._icon.opacity = 255;
        } else {
          this._icon.opacity = 140;
        }
      });

      // 4. Proper cleanup on destroy signal
      this.connect('destroy', () => {
        if (this._unsubscribeStore) {
          this._unsubscribeStore();
          this._unsubscribeStore = null;
        }
        if (this._unsubscribeDeviceStatus) {
          this._unsubscribeDeviceStatus();
          this._unsubscribeDeviceStatus = null;
        }
        if (this._hubWidget) {
          this._hubWidget.destroy();
        }
      });
    }

    private _updateBadge() {
      const count = this._store.getUnreadCount();
      if (count > 0) {
        this._badgeLabel.set_text(String(count));
        this._badgeLabel.show();
      } else {
        this._badgeLabel.set_text('');
        this._badgeLabel.hide();
      }
    }

    public openWithEvent(event: ActivityEvent) {
      this._store.setActiveItem(event);
      if (!this.menu.isOpen) {
        this.menu.open(true);
      }
    }
  }
);

export type BridgeIndicatorType = InstanceType<typeof BridgeIndicator>;
