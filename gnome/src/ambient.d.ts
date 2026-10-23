declare function log(...args: any[]): void;

declare module 'gi://St' {
  export namespace St {
    export interface WidgetParams {
      style_class?: string;
      style?: string;
      layout_manager?: any;
      x_expand?: boolean;
      y_expand?: boolean;
      reactive?: boolean;
      can_focus?: boolean;
      track_hover?: boolean;
      x_align?: any;
      y_align?: any;
      width?: number;
      height?: number;
      visible?: boolean;
      opacity?: number;
    }

    export class Widget {
      constructor(params?: WidgetParams);
      width: number;
      height: number;
      visible: boolean;
      opacity: number;
      reactive: boolean;
      can_focus: boolean;
      track_hover: boolean;
      style: string;
      style_class: string;
      x_align: any;
      y_align: any;
      x_expand: boolean;
      y_expand: boolean;
      add_child(child: any): void;
      remove_child(child: any): void;
      remove_all_children(): void;
      get_children(): any[];
      get_parent(): any;
      destroy(): void;
      show(): void;
      hide(): void;
      set_position(x: number, y: number): void;
      connect(signal: string, callback: (...args: any[]) => any): number;
      disconnect(id: number): void;
      add_style_class_name(name: string): void;
      remove_style_class_name(name: string): void;
      has_style_class_name(name: string): boolean;
      ease(properties: Record<string, any>, params?: Record<string, any>): void;
    }

    export class BoxLayout extends Widget {
      vertical: boolean;
      constructor(params?: WidgetParams & {
        vertical?: boolean;
      });
    }

    export class Label extends Widget {
      text: string;
      constructor(params?: WidgetParams & {
        text?: string;
      });
      set_text(text: string): void;
      get_text(): string;
    }

    export class Icon extends Widget {
      icon_name: string;
      icon_size: number;
      constructor(params?: WidgetParams & {
        icon_name?: string;
        icon_size?: number;
        gicon?: any;
      });
    }

    export class Button extends Widget {
      label?: string;
      constructor(params?: WidgetParams & {
        label?: string;
        child?: any;
      });
      set_child(child: any): void;
    }

    export class Entry extends Widget {
      text: string;
      hint_text: string;
      constructor(params?: WidgetParams & {
        text?: string;
        hint_text?: string;
      });
      set_text(text: string): void;
      get_text(): string;
      get_clutter_text(): any;
    }

    export class ScrollView extends Widget {
      constructor(params?: WidgetParams & {
        hscrollbar_policy?: number;
        vscrollbar_policy?: number;
        overlay_scrollbars?: boolean;
      });
      set_policy(hpolicy: number, vpolicy: number): void;
    }

    export enum PolicyType {
      ALWAYS = 0,
      AUTOMATIC = 1,
      NEVER = 2,
      EXTERNAL = 3,
    }

    export enum Align {
      START = 0,
      MIDDLE = 1,
      END = 2,
    }
  }
  export default St;
}

declare module 'gi://Clutter' {
  export namespace Clutter {
    export enum Orientation {
      HORIZONTAL = 0,
      VERTICAL = 1,
    }

    export enum AnimationMode {
      EASE_OUT_QUAD = 2,
      EASE_IN_OUT_QUAD = 3,
      EASE_OUT_CUBIC = 4,
    }

    export enum ActorAlign {
      FILL = 0,
      START = 1,
      CENTER = 2,
      END = 3,
    }

    export class BinLayout {
      constructor();
    }

    export class Actor {
      constructor(params?: any);
      width: number;
      height: number;
      opacity: number;
      visible: boolean;
      reactive: boolean;
      add_child(child: any): void;
      remove_child(child: any): void;
      destroy(): void;
      show(): void;
      hide(): void;
      set_position(x: number, y: number): void;
      connect(signal: string, callback: (...args: any[]) => any): number;
      disconnect(id: number): void;
      ease(properties: Record<string, any>, params?: Record<string, any>): void;
    }

    export const EVENT_STOP: boolean;
    export const EVENT_PROPAGATE: boolean;
  }
  export default Clutter;
}

declare module 'gi://GLib' {
  export namespace GLib {
    export const SOURCE_REMOVE: boolean;
    export const SOURCE_CONTINUE: boolean;
    export function timeout_add(priority: number, intervalMs: number, callback: () => boolean): number;
    export function timeout_add_seconds(priority: number, intervalSecs: number, callback: () => boolean): number;
    export function source_remove(id: number): boolean;
    export const PRIORITY_DEFAULT: number;
    export function get_home_dir(): string;
    export function get_current_dir(): string;
    export function getenv(variable: string): string | null;
    export function file_test(filename: string, test: number): boolean;
    export const FileTest: {
      IS_REGULAR: number;
      IS_SYMLINK: number;
      IS_DIR: number;
      IS_EXECUTABLE: number;
      EXISTS: number;
    };
    export class DateTime {
      static new_now_local(): DateTime;
      format(format: string): string;
    }
  }
  export default GLib;
}

declare module 'gi://Gio' {
  export namespace Gio {
    export class ThemedIcon {
      constructor(params: { name: string });
      static new(name: string): ThemedIcon;
    }
    export class File {
      static new_for_path(path: string): File;
    }
    export class FileIcon {
      static new(file: File): FileIcon;
    }
    export class Cancellable {
      constructor();
      cancel(): void;
      is_cancelled(): boolean;
    }
    export class SocketAddress {}
    export class UnixSocketAddress extends SocketAddress {
      static new(path: string): UnixSocketAddress;
    }
    export class SocketConnection {
      get_input_stream(): any;
      get_output_stream(): any;
      close(cancellable?: any): boolean;
    }
    export class SocketClient {
      constructor();
      connect_async(address: any, cancellable: any, callback: (source: any, res: any) => void): void;
      connect_finish(res: any): SocketConnection;
    }
    export class DataInputStream {
      constructor(params?: { base_stream: any });
      static new(base_stream: any): DataInputStream;
      read_line_async(priority: number, cancellable: any, callback: (source: any, res: any) => void): void;
      read_line_finish_utf8(res: any): [string | null, number];
      close(cancellable?: any): boolean;
    }
  }
  export default Gio;
}

declare module 'gi://GObject' {
  export namespace GObject {
    export function registerClass<T>(optionsOrClass: any, klass?: any): any;
    export class Object {
      connect(signal: string, callback: (...args: any[]) => any): number;
      disconnect(id: number): void;
    }
  }
  export default GObject;
}

declare module 'resource:///org/gnome/shell/extensions/extension.js' {
  export class Extension {
    metadata: {
      uuid: string;
      name: string;
      path: string;
      version?: number;
      'shell-version'?: string[];
      [key: string]: any;
    };
    dir: any;
    path: string;
    uuid: string;
    getSettings(schema?: string): any;
    enable(): void;
    disable(): void;
  }
  export function gettext(str: string): string;
}

declare module 'resource:///org/gnome/shell/ui/main.js' {
  export const panel: {
    height?: number;
    statusArea: Record<string, any>;
    addToStatusArea(role: string, indicator: any, position?: number, box?: string): any;
  };
  export const layoutManager: {
    uiGroup: any;
    primaryMonitor: {
      x: number;
      y: number;
      width: number;
      height: number;
    };
  };
  export const uiGroup: any;
  export function notify(msg: string, details?: string): void;
}

declare module 'resource:///org/gnome/shell/ui/panelMenu.js' {
  import St from 'gi://St';
  import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
  
  export class Button extends St.Widget {
    menu: PopupMenu.PopupMenu;
    constructor(...args: any[]);
    _init(...args: any[]): void;
  }
}

declare module 'resource:///org/gnome/shell/ui/popupMenu.js' {
  import St from 'gi://St';
  
  export class PopupMenu {
    box: St.BoxLayout;
    actor: St.Widget;
    isOpen: boolean;
    open(animate?: boolean): void;
    close(animate?: boolean): void;
    toggle(): void;
    addMenuItem(item: any, position?: number): void;
    removeAll(): void;
    destroy(): void;
    connect(signal: string, callback: (...args: any[]) => any): number;
    disconnect(id: number): void;
  }

  export class PopupBaseMenuItem extends St.Widget {
    actor: St.Widget;
    constructor(params?: any);
    _init(params?: any): void;
    add_child(child: any): void;
    destroy(): void;
  }

  export class PopupSeparatorMenuItem extends PopupBaseMenuItem {}
  export class PopupMenuItem extends PopupBaseMenuItem {
    label: St.Label;
    constructor(text: string, params?: any);
  }
  export class PopupSubMenuMenuItem extends PopupBaseMenuItem {}
}
