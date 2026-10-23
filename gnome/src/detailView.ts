import St from 'gi://St';
import Clutter from 'gi://Clutter';
import { ActivityEvent, IActivityStore } from './types.js';
import { getIconForType, getColorClassForApp, createActivityIcon } from './activityItem.js';

export class DetailViewWidget {
  public actor: St.BoxLayout;
  private _store: IActivityStore;
  private _currentEvent: ActivityEvent | null = null;
  private _onClose: (() => void) | null = null;

  constructor(store: IActivityStore, onClose?: () => void) {
    this._store = store;
    this._onClose = onClose || null;

    this.actor = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-detail-container',
      x_expand: true,
      y_expand: true,
    });

    this.render();
  }

  public setEvent(event: ActivityEvent | null) {
    this._currentEvent = event;
    this.render();
  }

  public render() {
    this.actor.remove_all_children();

    if (!this._currentEvent) {
      this._renderEmptyState();
      return;
    }

    switch (this._currentEvent.type) {
      case 'MESSAGE':
        this._renderMessageDetail(this._currentEvent);
        break;
      case 'CALL':
        this._renderCallDetail(this._currentEvent);
        break;
      case 'SCHOOL':
      case 'EMAIL':
      default:
        this._renderGenericDetail(this._currentEvent);
        break;
    }
  }

  private _renderEmptyState() {
    const emptyBox = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-detail-empty',
      x_expand: true,
      y_expand: true,
      x_align: Clutter.ActorAlign.CENTER,
      y_align: Clutter.ActorAlign.CENTER,
    });

    const icon = new St.Icon({
      icon_name: 'dialog-information-symbolic',
      icon_size: 36,
      style_class: 'bridge-detail-empty-icon',
    });
    emptyBox.add_child(icon);

    const label = new St.Label({
      text: 'Pilih notifikasi untuk melihat detail',
      style_class: 'bridge-detail-empty-text',
    });
    emptyBox.add_child(label);

    this.actor.add_child(emptyBox);
  }

  private _buildHeader(titleText: string, iconWidget?: St.Widget | null): St.BoxLayout {
    const header = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-detail-header',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });

    if (iconWidget) {
      header.add_child(iconWidget);
    }

    const titleLabel = new St.Label({
      text: titleText,
      style_class: 'bridge-detail-header-title',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });
    header.add_child(titleLabel);

    if (this._onClose) {
      const closeBtn = new St.Button({
        style_class: 'bridge-detail-close-button',
        reactive: true,
        can_focus: true,
        track_hover: true,
        y_align: Clutter.ActorAlign.CENTER,
      });

      const closeBox = new St.BoxLayout({
        vertical: false,
        style_class: 'bridge-detail-close-box',
        y_align: Clutter.ActorAlign.CENTER,
      });

      const closeIcon = new St.Icon({
        icon_name: 'window-close-symbolic',
        icon_size: 13,
        style_class: 'bridge-detail-close-icon',
      });
      closeBox.add_child(closeIcon);

      const closeLabel = new St.Label({
        text: 'Close',
        style_class: 'bridge-detail-close-label',
        y_align: Clutter.ActorAlign.CENTER,
      });
      closeBox.add_child(closeLabel);

      closeBtn.set_child(closeBox);

      closeBtn.connect('clicked', () => {
        if (this._onClose) this._onClose();
      });

      header.add_child(closeBtn);
    }

    return header;
  }

  private _renderMessageDetail(event: ActivityEvent) {
    // 1. Header: App icon + Title + Close button
    const iconWrapper = new St.BoxLayout({
      style_class: `bridge-detail-header-icon-wrapper ${getColorClassForApp(event.app)}`,
      y_align: Clutter.ActorAlign.CENTER,
    });
    const icon = createActivityIcon(event.app, event.type, 18, event.iconName);
    iconWrapper.add_child(icon);

    const header = this._buildHeader(event.title || event.app, iconWrapper);
    this.actor.add_child(header);

    // 2. Chat history
    const messagesBox = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-detail-messages-box',
      x_expand: true,
    });

    const chatHistory = event.chatHistory || [
      { id: '1', sender: event.title, text: event.body, time: '14:10', isOutgoing: false },
      { id: '2', sender: 'Me', text: 'jam setengah 7', time: '14:11', isOutgoing: true },
      { id: '3', sender: event.title, text: 'ok', time: '14:12', isOutgoing: false },
    ];

    chatHistory.forEach((msg) => {
      const bubbleRow = new St.BoxLayout({
        vertical: false,
        x_expand: true,
        x_align: msg.isOutgoing ? Clutter.ActorAlign.END : Clutter.ActorAlign.START,
        style_class: `bridge-chat-bubble-row ${msg.isOutgoing ? 'bridge-chat-outgoing-row' : 'bridge-chat-incoming-row'}`,
      });

      const bubble = new St.BoxLayout({
        vertical: true,
        style_class: `bridge-chat-bubble ${msg.isOutgoing ? 'bridge-chat-bubble-outgoing' : 'bridge-chat-bubble-incoming'}`,
      });
      bubble.add_child(
        new St.Label({
          text: msg.text,
          style_class: 'bridge-chat-bubble-text',
        })
      );

      bubbleRow.add_child(bubble);
      messagesBox.add_child(bubbleRow);
    });

    this.actor.add_child(messagesBox);

    // 3. Input bar
    const inputBar = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-detail-input-bar',
      x_expand: true,
      y_align: Clutter.ActorAlign.CENTER,
    });

    const entry = new St.Entry({
      hint_text: 'Start message',
      style_class: 'bridge-detail-entry',
      can_focus: true,
      x_expand: true,
    });

    const sendBtn = new St.Button({
      style_class: 'bridge-detail-send-button',
      reactive: true,
      can_focus: true,
      track_hover: true,
    });

    const sendIcon = new St.Icon({
      icon_name: 'mail-send-symbolic',
      icon_size: 16,
      style_class: 'bridge-send-icon',
    });
    sendBtn.set_child(sendIcon);

    const handleSend = () => {
      const text = entry.get_text().trim();
      if (!text) return;
      this._store.sendReply(event.id, text);
      entry.set_text('');
      const updated = this._store.getEventById(event.id);
      if (updated) {
        this.setEvent(updated);
      }
    };

    entry.get_clutter_text().connect('activate', () => {
      handleSend();
      return Clutter.EVENT_STOP;
    });

    sendBtn.connect('clicked', () => {
      handleSend();
      return Clutter.EVENT_STOP;
    });

    inputBar.add_child(entry);
    inputBar.add_child(sendBtn);

    this.actor.add_child(inputBar);
  }

  private _renderCallDetail(event: ActivityEvent) {
    const header = this._buildHeader('Incoming Call');
    this.actor.add_child(header);

    const callCard = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-call-container',
      x_expand: true,
      y_expand: true,
      x_align: Clutter.ActorAlign.CENTER,
      y_align: Clutter.ActorAlign.CENTER,
    });

    const callIconWrapper = new St.BoxLayout({
      style_class: 'bridge-call-avatar-wrapper',
    });
    const callIcon = new St.Icon({
      icon_name: 'call-start-symbolic',
      icon_size: 42,
      style_class: 'bridge-call-avatar-icon',
    });
    callIconWrapper.add_child(callIcon);
    callCard.add_child(callIconWrapper);

    const callerName = new St.Label({
      text: event.title,
      style_class: 'bridge-call-caller-name',
    });
    callCard.add_child(callerName);

    const callSubtitle = new St.Label({
      text: event.body || 'Incoming call from Android device...',
      style_class: 'bridge-call-subtitle',
    });
    callCard.add_child(callSubtitle);

    const actionsRow = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-call-actions-row',
      x_align: Clutter.ActorAlign.CENTER,
    });

    const declineBtn = new St.Button({
      label: 'Decline',
      style_class: 'bridge-call-btn bridge-call-decline',
      reactive: true,
      can_focus: true,
      track_hover: true,
    });
    declineBtn.connect('clicked', () => {
      this._store.handleCallAction(event.id, 'decline');
      if (this._onClose) this._onClose();
    });

    const answerBtn = new St.Button({
      label: 'Answer',
      style_class: 'bridge-call-btn bridge-call-answer',
      reactive: true,
      can_focus: true,
      track_hover: true,
    });
    answerBtn.connect('clicked', () => {
      this._store.handleCallAction(event.id, 'answer');
    });

    actionsRow.add_child(declineBtn);
    actionsRow.add_child(answerBtn);

    callCard.add_child(actionsRow);
    this.actor.add_child(callCard);
  }

  private _renderGenericDetail(event: ActivityEvent) {
    const iconWrapper = new St.BoxLayout({
      style_class: `bridge-detail-header-icon-wrapper ${getColorClassForApp(event.app)}`,
      y_align: Clutter.ActorAlign.CENTER,
    });
    const icon = createActivityIcon(event.app, event.type, 18, event.iconName);
    iconWrapper.add_child(icon);

    const header = this._buildHeader(event.app, iconWrapper);
    this.actor.add_child(header);

    const contentBox = new St.BoxLayout({
      vertical: true,
      style_class: 'bridge-generic-detail-box',
      x_expand: true,
      y_expand: true,
    });

    const title = new St.Label({
      text: event.title,
      style_class: 'bridge-generic-detail-title',
    });
    contentBox.add_child(title);

    const time = new St.Label({
      text: event.timestamp,
      style_class: 'bridge-generic-detail-time',
    });
    contentBox.add_child(time);

    const body = new St.Label({
      text: event.body,
      style_class: 'bridge-generic-detail-body',
    });
    contentBox.add_child(body);

    const actionRow = new St.BoxLayout({
      vertical: false,
      style_class: 'bridge-generic-action-row',
    });

    const dismissBtn = new St.Button({
      label: 'Dismiss',
      style_class: 'bridge-generic-btn-secondary',
      reactive: true,
      can_focus: true,
    });
    dismissBtn.connect('clicked', () => {
      this._store.dismissEvent(event.id);
      if (this._onClose) this._onClose();
    });

    const readBtn = new St.Button({
      label: 'Tandai Selesai',
      style_class: 'bridge-generic-btn-primary',
      reactive: true,
      can_focus: true,
    });
    readBtn.connect('clicked', () => {
      this._store.markAsRead(event.id);
    });

    actionRow.add_child(dismissBtn);
    actionRow.add_child(readBtn);
    contentBox.add_child(actionRow);

    this.actor.add_child(contentBox);
  }
}
