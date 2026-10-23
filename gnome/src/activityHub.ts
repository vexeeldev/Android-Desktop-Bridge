import St from 'gi://St';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import {
  ActivityEvent,
  ActivityStoreListener,
  ActiveItemListener,
  DeviceStatus,
  DeviceStatusListener,
  IActivityProvider,
  IActivityStore,
  ChatMessage,
} from './types.js';
import { ActivityItemWidget } from './activityItem.js';
import { DetailViewWidget } from './detailView.js';

/**
 * Concrete ActivityStore implementation
 */
export class ActivityStore implements IActivityStore {
  private _events: ActivityEvent[] = [];
  private _activeItem: ActivityEvent | null = null;
  private _listeners: Set<ActivityStoreListener> = new Set();
  private _activeItemListeners: Set<ActiveItemListener> = new Set();

  public getEvents(): ActivityEvent[] {
    return [...this._events];
  }

  public getEventById(id: string): ActivityEvent | undefined {
    return this._events.find((e) => e.id === id);
  }

  public markAsRead(id: string) {
    const item = this.getEventById(id);
    if (item && item.unread) {
      item.unread = false;
      this._notifyListeners();
    }
  }

  public markAllAsRead() {
    let changed = false;
    for (const event of this._events) {
      if (event.unread) {
        event.unread = false;
        changed = true;
      }
    }
    if (changed) {
      this._notifyListeners();
    }
  }

  public dismissEvent(id: string) {
    const idx = this._events.findIndex((e) => e.id === id);
    if (idx !== -1) {
      this._events.splice(idx, 1);
      if (this._activeItem?.id === id) {
        this.setActiveItem(this._events[0] || null);
      }
      this._notifyListeners();
    }
  }

  public addEvent(event: ActivityEvent) {
    const existingIndex = this._events.findIndex((e) => e.id === event.id);
    if (existingIndex !== -1) {
      const existing = this._events[existingIndex];
      const updated: ActivityEvent = {
        ...existing,
        ...event,
        chatHistory: event.chatHistory || existing.chatHistory,
      };
      this._events.splice(existingIndex, 1);
      this._events.unshift(updated);
    } else {
      this._events.unshift(event);
    }
    this._notifyListeners();
  }

  public sendReply(id: string, text: string) {
    const event = this.getEventById(id);
    if (!event) return;

    if (!event.chatHistory) {
      event.chatHistory = [];
    }

    const newMsg: ChatMessage = {
      id: String(Date.now()),
      sender: 'Me',
      text,
      time: 'Just now',
      isOutgoing: true,
    };

    event.chatHistory.push(newMsg);
    event.unread = false;
    this._notifyListeners();
  }

  public clearChat(id: string) {
    const event = this.getEventById(id);
    if (!event) return;
    event.chatHistory = [];
    this._notifyListeners();
  }

  public handleCallAction(id: string, action: 'answer' | 'decline') {
    const event = this.getEventById(id);
    if (!event) return;

    if (action === 'decline') {
      this.dismissEvent(id);
    } else {
      event.body = 'Call in progress...';
      event.unread = false;
      this._notifyListeners();
    }
  }

  public getUnreadCount(): number {
    return this._events.filter((e) => e.unread).length;
  }

  public getActiveItem(): ActivityEvent | null {
    return this._activeItem;
  }

  public setActiveItem(event: ActivityEvent | null) {
    this._activeItem = event;
    if (event && event.unread) {
      event.unread = false;
      this._notifyListeners();
    }
    for (const l of this._activeItemListeners) {
      try {
        l(this._activeItem);
      } catch (err) {
        log(`Error in activeItem listener: ${err}`);
      }
    }
  }

  public subscribe(listener: ActivityStoreListener): () => void {
    this._listeners.add(listener);
    return () => this._listeners.delete(listener);
  }

  public subscribeActiveItem(listener: ActiveItemListener): () => void {
    this._activeItemListeners.add(listener);
    return () => this._activeItemListeners.delete(listener);
  }

  private _deviceStatus: DeviceStatus = {
    name: 'Android Connected',
    connected: true,
  };
  private _deviceStatusListeners: Set<DeviceStatusListener> = new Set();

  public getDeviceStatus(): DeviceStatus {
    return { ...this._deviceStatus };
  }

  public setDeviceStatus(status: DeviceStatus) {
    this._deviceStatus = { ...status };
    for (const listener of this._deviceStatusListeners) {
      try {
        listener(this._deviceStatus);
      } catch (err) {
        log(`Error in device status listener: ${err}`);
      }
    }
  }

  public subscribeDeviceStatus(listener: DeviceStatusListener): () => void {
    this._deviceStatusListeners.add(listener);
    listener(this._deviceStatus);
    return () => this._deviceStatusListeners.delete(listener);
  }

  private _notifyListeners() {
    const snapshot = this.getEvents();
    for (const listener of this._listeners) {
      try {
        listener(snapshot);
      } catch (err) {
        log(`Error in activity store listener: ${err}`);
      }
    }
  }
}

/**
 * Mock Activity Provider
 */
export class MockActivityProvider implements IActivityProvider {
  public name = 'MockActivityProvider';
  private _store: IActivityStore | null = null;
  private _timerId: number = 0;

  public initialize(store: IActivityStore) {
    this._store = store;

    // Load initial mock data matching Figma reference and user requirements
    const initialEvents: ActivityEvent[] = [
      {
        id: '1',
        type: 'MESSAGE',
        app: 'WhatsApp',
        title: 'Budi',
        body: 'ke kantor jam berapa ?',
        unread: true,
        timestamp: '14:12',
        chatHistory: [
          {
            id: 'm1',
            sender: 'Budi',
            text: 'ke kantor jam berapa ?',
            time: '14:10',
            isOutgoing: false,
          },
          {
            id: 'm2',
            sender: 'Me',
            text: 'jam setengah 7',
            time: '14:11',
            isOutgoing: true,
          },
          {
            id: 'm3',
            sender: 'Budi',
            text: 'ok',
            time: '14:12',
            isOutgoing: false,
          },
        ],
      },
      {
        id: '2',
        type: 'CALL',
        app: 'Phone',
        title: 'Budi',
        body: 'Incoming call',
        unread: true,
        timestamp: '14:15',
        callDetails: {
          caller: 'Budi',
          phoneNumber: '+62 812-3456-7890',
          callState: 'incoming',
          timestamp: '14:15',
        },
      },
      {
        id: '3',
        type: 'SCHOOL',
        app: 'School',
        title: 'Tugas baru',
        body: 'Matematika — deadline 18:00',
        unread: true,
        timestamp: '13:45',
      },
      {
        id: '4',
        type: 'EMAIL',
        app: 'Gmail',
        title: '2 pesan baru',
        body: 'Tim Bridge: Update release v1.0 telah diverifikasi.',
        unread: false,
        timestamp: '12:30',
      },
      {
        id: '5',
        type: 'BATTERY',
        app: 'System',
        title: 'Battery 85%',
        body: 'Charging (Fast Charge) — 25 min until full',
        unread: false,
        timestamp: '14:00',
      },
    ];

    for (const evt of initialEvents.reverse()) {
      store.addEvent(evt);
    }

    // Set first message as active item to match Figma master-detail screenshot
    store.setActiveItem(initialEvents[0]);
  }

  public simulateIncomingEvent(): ActivityEvent {
    if (!this._store) {
      throw new Error('Store not initialized');
    }

    const sampleSenders = ['Siti', 'Budi', 'Rian', 'Andi', 'Dian'];
    const sampleTexts = [
      'File dokumen sudah dikirim ya!',
      'Nanti makan siang bareng di mana?',
      'Meeting dimajukan ke jam 3 sore.',
      'Bisa tolong review PR-nya?',
      'Link server bridge sudah aktif.',
    ];

    const sender = sampleSenders[Math.floor(Math.random() * sampleSenders.length)];
    const text = sampleTexts[Math.floor(Math.random() * sampleTexts.length)];
    const id = String(Date.now());

    const newEvent: ActivityEvent = {
      id,
      type: 'MESSAGE',
      app: 'WhatsApp',
      title: sender,
      body: text,
      unread: true,
      timestamp: 'Sekarang',
      chatHistory: [
        {
          id: `msg-${id}`,
          sender,
          text,
          time: 'Sekarang',
          isOutgoing: false,
        },
      ],
    };

    this._store.addEvent(newEvent);
    return newEvent;
  }

  public destroy() {
    if (this._timerId) {
      GLib.source_remove(this._timerId);
      this._timerId = 0;
    }
    this._store = null;
  }
}

/**
 * ActivityHubWidget: Master-Detail container for the popup menu
 */
export class ActivityHubWidget {
  public actor: St.BoxLayout;
  private _store: IActivityStore;
  private _provider: IActivityProvider;
  private _listContainer: St.BoxLayout;
  private _detailView: DetailViewWidget;
  private _statusDot!: St.Icon;
  private _deviceNameLabel!: St.Label;
  private _unsubscribeStore: (() => void) | null = null;
  private _unsubscribeActive: (() => void) | null = null;
  private _unsubscribeDeviceStatus: (() => void) | null = null;
  private _onSimulateNotification?: () => void;

  constructor(
    store: IActivityStore,
    provider: IActivityProvider,
    onSimulateNotification?: () => void
  ) {
    this._store = store;
    this._provider = provider;
    this._onSimulateNotification = onSimulateNotification;

    this.actor = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-hub-panel',
    });

    // 1. Header Bar: "• V211 Connected" + action tools
    this._buildHeader();

    // 2. Dual-Pane Content Body
    const contentBody = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-hub-content-body',
      x_expand: true,
      y_expand: true,
    });

    // Left pane: activity list
    const leftPane = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-hub-left-pane',
    });

    const listScrollView = new St.ScrollView({
      style_class: 'bridge-hub-scroll-view',
      hscrollbar_policy: St.PolicyType.NEVER,
      vscrollbar_policy: St.PolicyType.AUTOMATIC,
      x_expand: true,
      y_expand: true,
    });

    this._listContainer = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-hub-list-box',
      x_expand: true,
    });

    listScrollView.add_child(this._listContainer);
    leftPane.add_child(listScrollView);
    contentBody.add_child(leftPane);

    // Right pane: detail view
    const rightPane = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-hub-right-pane',
      x_expand: true,
      y_expand: true,
    });

    this._detailView = new DetailViewWidget(this._store, () => {
      // Back/Close action handler
      this._store.setActiveItem(null);
    });

    rightPane.add_child(this._detailView.actor);
    contentBody.add_child(rightPane);

    this.actor.add_child(contentBody);

    // Initial render and subscriptions
    this._renderList();
    this._detailView.setEvent(this._store.getActiveItem());

    this._unsubscribeStore = this._store.subscribe(() => {
      this._renderList();
      const currentActive = this._store.getActiveItem();
      if (currentActive) {
        const fresh = this._store.getEventById(currentActive.id);
        if (fresh) {
          this._detailView.setEvent(fresh);
        }
      }
    });

    this._unsubscribeActive = this._store.subscribeActiveItem((event) => {
      this._renderList();
      this._detailView.setEvent(event);
    });
  }

  private _buildHeader() {
    const header = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-hub-header',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });

    // Device connection indicator: Green dot + "V211 Connected"
    const statusBox = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-device-status-box',
      y_align: Clutter.ActorAlign.CENTER,
    });

    const greenDot = new St.Icon({
      icon_name: 'media-record-symbolic',
      icon_size: 14,
      style_class: 'bridge-status-dot-green',
      y_align: Clutter.ActorAlign.CENTER,
    });
    statusBox.add_child(greenDot);
    this._statusDot = greenDot;

    const deviceName = new St.Label({
      text: 'Android Connected',
      style_class: 'bridge-device-title',
      y_align: Clutter.ActorAlign.CENTER,
    });
    statusBox.add_child(deviceName);
    this._deviceNameLabel = deviceName;

    this._unsubscribeDeviceStatus = this._store.subscribeDeviceStatus((status) => {
      this._deviceNameLabel.set_text(status.name);
      if (status.connected) {
        this._statusDot.style_class = 'bridge-status-dot-green';
      } else {
        this._statusDot.style_class = 'bridge-status-dot-red';
      }
    });

    header.add_child(statusBox);

    // Spacer
    const spacer = new St.BoxLayout({ x_expand: true });
    header.add_child(spacer);

    // Header actions (Simulate Event button + Settings gear)
    const actionsBox = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-header-actions',
      y_align: Clutter.ActorAlign.CENTER,
    });

    // Test simulation button
    // const simButton = new St.Button({
    //   label: 'Simulasi +',
    //   style_class: 'bridge-sim-button',
    //   reactive: true,
    //   can_focus: true,
    //   track_hover: true,
    // });
    // simButton.connect('clicked', () => {
    //   if (this._onSimulateNotification) {
    //     this._onSimulateNotification();
    //   } else {
    //     this._provider.simulateIncomingEvent();
    //   }
    //   return Clutter.EVENT_STOP;
    // });
    // actionsBox.add_child(simButton);

    // Settings gear button
    const settingsButton = new St.Button({
      style_class: 'bridge-settings-button',
      reactive: true,
      can_focus: true,
      track_hover: true,
    });
    const gearIcon = new St.Icon({
      icon_name: 'preferences-system-symbolic',
      icon_size: 16,
    });
    settingsButton.set_child(gearIcon);
    settingsButton.connect('clicked', () => {
      this._store.markAllAsRead();
      return Clutter.EVENT_STOP;
    });
    actionsBox.add_child(settingsButton);

    header.add_child(actionsBox);
    this.actor.add_child(header);
  }

  private _renderList() {
    this._listContainer.remove_all_children();
    const events = this._store.getEvents();
    const activeItem = this._store.getActiveItem();

    if (events.length === 0) {
      const emptyLabel = new St.Label({
        text: 'Tidak ada aktivitas baru',
        style_class: 'bridge-list-empty-label',
      });
      this._listContainer.add_child(emptyLabel);
      return;
    }

    events.forEach((event) => {
      const isSelected = activeItem ? activeItem.id === event.id : false;
      const itemWidget = new ActivityItemWidget(
        event,
        this._store,
        (selected) => {
          this._store.setActiveItem(selected);
        },
        isSelected
      );
      this._listContainer.add_child(itemWidget.actor);
    });
  }

  public destroy() {
    if (this._unsubscribeStore) this._unsubscribeStore();
    if (this._unsubscribeActive) this._unsubscribeActive();
    if (this._unsubscribeDeviceStatus) this._unsubscribeDeviceStatus();
    this.actor.destroy();
  }
}
