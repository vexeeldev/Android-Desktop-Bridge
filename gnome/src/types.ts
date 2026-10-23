/**
 * Bridge GNOME - Core Types and Interfaces
 */

export type ActivityType =
  | 'MESSAGE'
  | 'CALL'
  | 'NOTIFICATION'
  | 'SCHOOL'
  | 'EMAIL'
  | 'DOWNLOAD'
  | 'BATTERY'
  | 'SYSTEM';

export interface ChatMessage {
  id: string;
  sender: string;
  text: string;
  time: string;
  isOutgoing: boolean;
}

export interface CallDetails {
  caller: string;
  phoneNumber?: string;
  callState: 'incoming' | 'missed' | 'active' | 'ended';
  timestamp: string;
}

export interface ActivityEvent {
  id: string;
  type: ActivityType;
  app: string;
  title: string;
  body: string;
  unread: boolean;
  timestamp: string;
  iconName?: string;
  chatHistory?: ChatMessage[];
  callDetails?: CallDetails;
  actionText?: string;
  isOngoing?: boolean;
  isClearable?: boolean;
  canReply?: boolean;
}

export interface DeviceStatus {
  name: string;
  connected: boolean;
  batteryLevel?: number;
  isCharging?: boolean;
  deviceId?: string;
  ipAddress?: string;
}

/**
 * Listener callback signature when activity store changes
 */
export type ActivityStoreListener = (events: ActivityEvent[]) => void;
export type ActiveItemListener = (event: ActivityEvent | null) => void;
export type DeviceStatusListener = (status: DeviceStatus) => void;

/**
 * Activity Store Interface
 */
export interface IActivityStore {
  getEvents(): ActivityEvent[];
  getEventById(id: string): ActivityEvent | undefined;
  markAsRead(id: string): void;
  markAllAsRead(): void;
  dismissEvent(id: string): void;
  addEvent(event: ActivityEvent): void;
  sendReply(id: string, text: string): void;
  clearChat(id: string): void;
  handleCallAction(id: string, action: 'answer' | 'decline'): void;
  subscribe(listener: ActivityStoreListener): () => void;
  getUnreadCount(): number;
  getActiveItem(): ActivityEvent | null;
  setActiveItem(event: ActivityEvent | null): void;
  subscribeActiveItem(listener: ActiveItemListener): () => void;
  getDeviceStatus(): DeviceStatus;
  setDeviceStatus(status: DeviceStatus): void;
  subscribeDeviceStatus(listener: DeviceStatusListener): () => void;
}

/**
 * Activity Provider Interface (e.g. MockProvider, BridgeDaemonProvider)
 */
export interface IActivityProvider {
  name: string;
  initialize(store: IActivityStore, onNotification?: (event: ActivityEvent) => void): void;
  destroy(): void;
  simulateIncomingEvent?(): ActivityEvent;
}
