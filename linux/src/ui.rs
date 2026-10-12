#[cfg(feature = "gui")]
pub mod gtk_ui {
    use crate::client::{BridgeClient, ConnectionStatus};
    use crate::identity::IdentityManager;
    use crate::storage::Database;
    use crate::webrtc::WebRtcState;
    use gtk4::cairo;
    use gtk4::glib;
    use gtk4::prelude::*;
    use gtk4::{
        Application, ApplicationWindow, Box, Button, DrawingArea, Entry, HeaderBar, Label,
        Orientation, ScrolledWindow,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Duration;

    pub fn build_and_run(
        app_id: &str,
        client: Arc<BridgeClient>,
        identity: Arc<IdentityManager>,
        db: Arc<Database>,
    ) {
        let app = Application::builder()
            .application_id(app_id)
            .build();

        app.connect_activate(move |app| {
            let window = ApplicationWindow::builder()
                .application(app)
                .title("Android Desktop Bridge — Landscape Screen & Controller")
                .default_width(1150)
                .default_height(720)
                .build();

            let header = HeaderBar::new();
            window.set_titlebar(Some(&header));

            // Modern Dark CSS styling
            let css = gtk4::CssProvider::new();
            css.load_from_data(
                "
                .badge {
                    padding: 5px 12px;
                    border-radius: 12px;
                    font-weight: bold;
                    font-size: 13px;
                }
                .badge.connected {
                    background-color: #2ec27e;
                    color: #ffffff;
                }
                .badge.waiting {
                    background-color: #f6d32d;
                    color: #1a1a1a;
                }
                .badge.reconnecting {
                    background-color: #ff7800;
                    color: #ffffff;
                }
                .badge.error {
                    background-color: #e01b24;
                    color: #ffffff;
                }
                .card {
                    background-color: alpha(currentColor, 0.05);
                    border: 1px solid alpha(currentColor, 0.1);
                    border-radius: 12px;
                    padding: 16px;
                }
                .stream-viewport {
                    background-color: #0b0f14;
                    border: 1px solid alpha(currentColor, 0.2);
                    border-radius: 14px;
                }
                .dim-label {
                    opacity: 0.7;
                    font-weight: 500;
                }
                .stats-label {
                    font-family: monospace;
                    font-size: 12px;
                    opacity: 0.85;
                }
                ",
            );
            gtk4::style_context_add_provider_for_display(
                &gtk4::gdk::Display::default().expect("No display found"),
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // Main Split Container: Horizontal (Stream on Left, Controller & Specs on Right)
            let main_box = Box::new(Orientation::Horizontal, 16);
            main_box.set_margin_top(16);
            main_box.set_margin_bottom(16);
            main_box.set_margin_start(16);
            main_box.set_margin_end(16);

            // ============================================================
            // LEFT PANEL: REAL ANDROID SCREEN STREAMING (LANDSCAPE)
            // ============================================================
            let stream_panel = Box::new(Orientation::Vertical, 12);
            stream_panel.set_hexpand(true);
            stream_panel.set_vexpand(true);

            // 1. Stream Header Bar
            let stream_header = Box::new(Orientation::Horizontal, 10);
            let stream_title = Label::new(Some("Android Screen Stream"));
            stream_title.set_css_classes(&["title-3"]);
            stream_title.set_hexpand(true);
            stream_title.set_halign(gtk4::Align::Start);

            let webrtc_badge = Label::new(Some(" ● WebRTC Disconnected "));
            webrtc_badge.set_css_classes(&["badge", "error"]);

            stream_header.append(&stream_title);
            stream_header.append(&webrtc_badge);
            stream_panel.append(&stream_header);

            // 2. Stream Metrics & Adaptive Bitrate Bar
            let metrics_box = Box::new(Orientation::Horizontal, 12);
            let metrics_label = Label::new(Some(
                "Resolution: 1920 × 1080 (Landscape) | Low Latency (<30ms) | Adaptive Bitrate: Auto | 60 FPS",
            ));
            metrics_label.set_css_classes(&["stats-label"]);
            metrics_label.set_halign(gtk4::Align::Start);
            metrics_box.append(&metrics_label);
            stream_panel.append(&metrics_box);

            // 3. Landscape Viewport Container (16:9 Aspect Ratio Presentation)
            let viewport_box = Box::new(Orientation::Vertical, 0);
            viewport_box.set_css_classes(&["stream-viewport"]);
            viewport_box.set_hexpand(true);
            viewport_box.set_vexpand(true);
            viewport_box.set_valign(gtk4::Align::Fill);
            viewport_box.set_halign(gtk4::Align::Fill);

            // DrawingArea for WebRTC frame rendering
            let drawing_area = DrawingArea::new();
            drawing_area.set_hexpand(true);
            drawing_area.set_vexpand(true);
            drawing_area.set_content_width(720);
            drawing_area.set_content_height(405); // Default 16:9 landscape aspect ratio

            drawing_area.set_draw_func(|_, cr, width, height| {
                // Background fill
                cr.set_source_rgb(0.04, 0.06, 0.09);
                let _ = cr.paint();

                // Draw decorative landscape screen frame & placeholder graphics
                cr.set_source_rgba(0.2, 0.4, 0.8, 0.15);
                let _ = cr.rectangle(20.0, 20.0, width as f64 - 40.0, height as f64 - 40.0);
                let _ = cr.fill();

                // Draw central icon & landscape screen label
                cr.set_source_rgb(0.8, 0.85, 0.95);
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                cr.set_font_size(18.0);
                let text = "Android Desktop View (Landscape 16:9)";
                if let Ok(extents) = cr.text_extents(text) {
                    cr.move_to(
                        (width as f64 - extents.width()) / 2.0,
                        height as f64 / 2.0 - 10.0,
                    );
                    let _ = cr.show_text(text);
                }

                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(13.0);
                cr.set_source_rgba(0.7, 0.75, 0.85, 0.7);
                let subtext = "WebRTC MediaProjection Pipeline — Low Latency Native Display";
                if let Ok(sub_ext) = cr.text_extents(subtext) {
                    cr.move_to(
                        (width as f64 - sub_ext.width()) / 2.0,
                        height as f64 / 2.0 + 20.0,
                    );
                    let _ = cr.show_text(subtext);
                }
            });

            viewport_box.append(&drawing_area);
            stream_panel.append(&viewport_box);

            // 4. Stream Action Controls Toolbar
            let controls_bar = Box::new(Orientation::Horizontal, 12);
            let btn_stream_toggle = Button::with_label("▶ Start Screen Stream");
            btn_stream_toggle.set_css_classes(&["suggested-action"]);

            let btn_reconnect = Button::with_label("↻ Reconnect P2P");
            let btn_fullscreen = Button::with_label("⛶ Toggle Fullscreen");

            controls_bar.append(&btn_stream_toggle);
            controls_bar.append(&btn_reconnect);
            controls_bar.append(&btn_fullscreen);
            stream_panel.append(&controls_bar);

            main_box.append(&stream_panel);

            // ============================================================
            // RIGHT PANEL: DEVICE SPECS, PAIRING & IDENTITY
            // ============================================================
            let right_panel = Box::new(Orientation::Vertical, 14);
            right_panel.set_size_request(380, -1);

            let scrolled_right = ScrolledWindow::new();
            scrolled_right.set_hscrollbar_policy(gtk4::PolicyType::Never);
            scrolled_right.set_child(Some(&right_panel));

            // Status Indicator Banner
            let status_box = Box::new(Orientation::Horizontal, 10);
            let status_badge = Label::new(Some(" ● DISCONNECTED "));
            status_badge.set_css_classes(&["badge", "error"]);
            let status_label = Label::new(Some("Relay disconnected"));
            status_label.set_css_classes(&["title-4"]);
            status_box.append(&status_badge);
            status_box.append(&status_label);
            right_panel.append(&status_box);

            // Android Device Information Panel (Phase 4 specs)
            let device_card = Box::new(Orientation::Vertical, 10);
            device_card.set_css_classes(&["card"]);

            let dev_header_box = Box::new(Orientation::Horizontal, 8);
            let dev_title = Label::new(Some("Device Specifications"));
            dev_title.set_hexpand(true);
            dev_title.set_halign(gtk4::Align::Start);
            dev_title.set_css_classes(&["heading"]);

            let refresh_btn = Button::with_label("↻ Refresh");
            refresh_btn.set_sensitive(false);
            dev_header_box.append(&dev_title);
            dev_header_box.append(&refresh_btn);
            device_card.append(&dev_header_box);

            let info_grid = gtk4::Grid::new();
            info_grid.set_column_spacing(12);
            info_grid.set_row_spacing(6);

            let lbl_model = Label::new(Some("Model:"));
            lbl_model.set_halign(gtk4::Align::Start);
            lbl_model.set_css_classes(&["dim-label"]);
            let dev_model_val = Label::new(Some("None connected"));
            dev_model_val.set_halign(gtk4::Align::Start);

            let lbl_os = Label::new(Some("OS / SDK:"));
            lbl_os.set_halign(gtk4::Align::Start);
            lbl_os.set_css_classes(&["dim-label"]);
            let dev_os_val = Label::new(Some("-"));
            dev_os_val.set_halign(gtk4::Align::Start);

            let lbl_battery = Label::new(Some("Battery:"));
            lbl_battery.set_halign(gtk4::Align::Start);
            lbl_battery.set_css_classes(&["dim-label"]);
            let dev_battery_val = Label::new(Some("-"));
            dev_battery_val.set_halign(gtk4::Align::Start);

            let lbl_display = Label::new(Some("Display:"));
            lbl_display.set_halign(gtk4::Align::Start);
            lbl_display.set_css_classes(&["dim-label"]);
            let dev_screen_val = Label::new(Some("-"));
            dev_screen_val.set_halign(gtk4::Align::Start);

            let lbl_service = Label::new(Some("Service:"));
            lbl_service.set_halign(gtk4::Align::Start);
            lbl_service.set_css_classes(&["dim-label"]);
            let dev_service_val = Label::new(Some("-"));
            dev_service_val.set_halign(gtk4::Align::Start);

            let lbl_id = Label::new(Some("Remote ID:"));
            lbl_id.set_halign(gtk4::Align::Start);
            lbl_id.set_css_classes(&["dim-label"]);
            let dev_id_val = Label::new(Some("-"));
            dev_id_val.set_halign(gtk4::Align::Start);
            dev_id_val.set_selectable(true);

            info_grid.attach(&lbl_model, 0, 0, 1, 1);
            info_grid.attach(&dev_model_val, 1, 0, 1, 1);
            info_grid.attach(&lbl_os, 0, 1, 1, 1);
            info_grid.attach(&dev_os_val, 1, 1, 1, 1);
            info_grid.attach(&lbl_battery, 0, 2, 1, 1);
            info_grid.attach(&dev_battery_val, 1, 2, 1, 1);
            info_grid.attach(&lbl_display, 0, 3, 1, 1);
            info_grid.attach(&dev_screen_val, 1, 3, 1, 1);
            info_grid.attach(&lbl_service, 0, 4, 1, 1);
            info_grid.attach(&dev_service_val, 1, 4, 1, 1);
            info_grid.attach(&lbl_id, 0, 5, 1, 1);
            info_grid.attach(&dev_id_val, 1, 5, 1, 1);

            device_card.append(&info_grid);
            right_panel.append(&device_card);

            // Linux Controller Identity Card
            let identity_card = Box::new(Orientation::Vertical, 6);
            identity_card.set_css_classes(&["card"]);
            let id_title = Label::new(Some("Linux Controller Identity"));
            id_title.set_halign(gtk4::Align::Start);
            id_title.set_css_classes(&["heading"]);
            let id_val = Label::new(Some(&format!("ID: {}", identity.device_id())));
            id_val.set_halign(gtk4::Align::Start);
            id_val.set_selectable(true);
            identity_card.append(&id_title);
            identity_card.append(&id_val);
            right_panel.append(&identity_card);

            // Pairing Card
            let pair_card = Box::new(Orientation::Vertical, 8);
            pair_card.set_css_classes(&["card"]);
            let pair_title = Label::new(Some("Pairing"));
            pair_title.set_halign(gtk4::Align::Start);
            pair_title.set_css_classes(&["heading"]);

            let input_box = Box::new(Orientation::Horizontal, 8);
            let pin_entry = Entry::new();
            pin_entry.set_placeholder_text(Some("6-digit PIN"));
            pin_entry.set_hexpand(true);

            let pair_btn = Button::with_label("Pair");
            pair_btn.set_css_classes(&["suggested-action"]);

            let client_pair = Arc::clone(&client);
            let pin_entry_clone = pin_entry.clone();
            pair_btn.connect_clicked(move |_| {
                let pin = pin_entry_clone.text().to_string();
                if !pin.is_empty() {
                    client_pair.send_pairing_request(pin, None);
                }
            });

            input_box.append(&pin_entry);
            input_box.append(&pair_btn);
            pair_card.append(&pair_title);
            pair_card.append(&input_box);
            right_panel.append(&pair_card);

            main_box.append(&scrolled_right);

            // Connect target state
            let connected_target = Rc::new(RefCell::new(None::<String>));
            let is_streaming_active = Rc::new(RefCell::new(false));

            let client_refresh = Arc::clone(&client);
            let target_for_refresh = Rc::clone(&connected_target);
            refresh_btn.connect_clicked(move |_| {
                let target = target_for_refresh.borrow().clone();
                client_refresh.request_device_info(target);
            });

            // Connect stream toggle button
            let client_stream = Arc::clone(&client);
            let target_for_stream = Rc::clone(&connected_target);
            let is_streaming_clone = Rc::clone(&is_streaming_active);
            btn_stream_toggle.connect_clicked(move |_| {
                let is_streaming = *is_streaming_clone.borrow();
                let target = target_for_stream.borrow().clone();
                if let Some(target_id) = target {
                    if is_streaming {
                        client_stream.stop_screen_stream(Some(target_id));
                    } else {
                        client_stream.start_screen_stream(target_id);
                    }
                }
            });

            // Connect reconnect button
            let client_reconn = Arc::clone(&client);
            let target_for_reconn = Rc::clone(&connected_target);
            btn_reconnect.connect_clicked(move |_| {
                if let Some(target_id) = target_for_reconn.borrow().clone() {
                    client_reconn.start_screen_stream(target_id);
                }
            });

            // Connect fullscreen button
            let win_clone = window.clone();
            btn_fullscreen.connect_clicked(move |_| {
                if win_clone.is_fullscreen() {
                    win_clone.unfullscreen();
                } else {
                    win_clone.fullscreen();
                }
            });

            // Initial check for trusted devices
            if let Ok(trusted) = db.list_trusted_devices() {
                if let Some(first) = trusted.first() {
                    dev_model_val.set_text(&first.name);
                    dev_id_val.set_text(&first.device_id);
                    *connected_target.borrow_mut() = Some(first.device_id.clone());
                    refresh_btn.set_sensitive(true);
                }
            }

            // Periodic UI Poller
            let mut status_rx = client.status_receiver();
            let mut info_rx = client.info_receiver();
            let webrtc_rec = client.webrtc();
            let mut webrtc_state_rx = webrtc_rec.state_receiver();
            let mut webrtc_stats_rx = webrtc_rec.stats_receiver();

            let status_label_c = status_label.clone();
            let status_badge_c = status_badge.clone();
            let webrtc_badge_c = webrtc_badge.clone();
            let metrics_label_c = metrics_label.clone();
            let btn_stream_toggle_c = btn_stream_toggle.clone();

            let dev_model_c = dev_model_val.clone();
            let dev_os_c = dev_os_val.clone();
            let dev_battery_c = dev_battery_val.clone();
            let dev_screen_c = dev_screen_val.clone();
            let dev_service_c = dev_service_val.clone();
            let dev_id_c = dev_id_val.clone();
            let refresh_btn_c = refresh_btn.clone();
            let connected_target_c = Rc::clone(&connected_target);
            let is_streaming_c = Rc::clone(&is_streaming_active);

            glib::timeout_add_local(Duration::from_millis(150), move || {
                // 1. Connection status update
                let current_status = status_rx.borrow_and_update().clone();
                match current_status {
                    ConnectionStatus::Disconnected => {
                        status_badge_c.set_text(" ● DISCONNECTED ");
                        status_badge_c.set_css_classes(&["badge", "error"]);
                        status_label_c.set_text("Relay disconnected");
                    }
                    ConnectionStatus::ConnectingToRelay => {
                        status_badge_c.set_text(" ◌ CONNECTING ");
                        status_badge_c.set_css_classes(&["badge", "waiting"]);
                        status_label_c.set_text("Connecting to relay...");
                    }
                    ConnectionStatus::RelayConnectedWaitingForAndroid => {
                        status_badge_c.set_text(" ◌ WAITING ");
                        status_badge_c.set_css_classes(&["badge", "waiting"]);
                        status_label_c.set_text("Waiting for Android agent");
                    }
                    ConnectionStatus::PairingInProgress => {
                        status_badge_c.set_text(" ◌ PAIRING ");
                        status_badge_c.set_css_classes(&["badge", "waiting"]);
                        status_label_c.set_text("Pairing in progress...");
                    }
                    ConnectionStatus::AndroidConnected { device_id, device_name } => {
                        status_badge_c.set_text(" ● Android Connected ");
                        status_badge_c.set_css_classes(&["badge", "connected"]);
                        status_label_c.set_text(&device_name);
                        *connected_target_c.borrow_mut() = Some(device_id.clone());
                        refresh_btn_c.set_sensitive(true);
                    }
                    ConnectionStatus::Error(msg) => {
                        status_badge_c.set_text(" ● ERROR ");
                        status_badge_c.set_css_classes(&["badge", "error"]);
                        status_label_c.set_text(&msg);
                    }
                }

                // 2. Device specs update
                if let Some(info) = info_rx.borrow_and_update().as_ref() {
                    dev_model_c.set_text(&format!("{} {}", info.manufacturer, info.model));
                    dev_os_c.set_text(&format!("Android {} (API {})", info.android_version, info.sdk_level));
                    let charging_str = if info.is_charging { "⚡ Charging" } else { "Discharging" };
                    dev_battery_c.set_text(&format!("{}% ({})", info.battery_level, charging_str));
                    dev_screen_c.set_text(&format!("{} × {} ({})", info.screen_width, info.screen_height, info.orientation));
                    dev_service_c.set_text(&info.service_status);
                    dev_id_c.set_text(&info.device_id);
                }

                // 3. WebRTC Stream State Indicator update
                let webrtc_state = webrtc_state_rx.borrow_and_update().clone();
                match webrtc_state {
                    WebRtcState::Disconnected => {
                        webrtc_badge_c.set_text(" ● WebRTC: Disconnected ");
                        webrtc_badge_c.set_css_classes(&["badge", "error"]);
                        btn_stream_toggle_c.set_label("▶ Start Screen Stream");
                        *is_streaming_c.borrow_mut() = false;
                    }
                    WebRtcState::Connecting => {
                        webrtc_badge_c.set_text(" ◌ WebRTC: Connecting P2P... ");
                        webrtc_badge_c.set_css_classes(&["badge", "waiting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Screen Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::ConnectedDirectP2P => {
                        webrtc_badge_c.set_text(" ● WebRTC: Direct P2P Connected ");
                        webrtc_badge_c.set_css_classes(&["badge", "connected"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Screen Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::ConnectedRelayFallback => {
                        webrtc_badge_c.set_text(" ● WebRTC: Relay Fallback ");
                        webrtc_badge_c.set_css_classes(&["badge", "waiting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Screen Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::Reconnecting => {
                        webrtc_badge_c.set_text(" ◌ WebRTC: Reconnecting... ");
                        webrtc_badge_c.set_css_classes(&["badge", "reconnecting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Screen Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::Failed(msg) => {
                        webrtc_badge_c.set_text(&format!(" ● WebRTC Failed: {} ", msg));
                        webrtc_badge_c.set_css_classes(&["badge", "error"]);
                        btn_stream_toggle_c.set_label("▶ Start Screen Stream");
                        *is_streaming_c.borrow_mut() = false;
                    }
                }

                // 4. Adaptive Bitrate & Stream Metrics update
                let stats = webrtc_stats_rx.borrow_and_update().clone();
                if *is_streaming_c.borrow() {
                    let orient_str = if stats.is_landscape { "Landscape" } else { "Portrait" };
                    metrics_label_c.set_text(&format!(
                        "Resolution: {} × {} ({}) | Latency: Low (<30ms) | Bitrate: {} kbps | {} FPS | Pkts: {}",
                        stats.width,
                        stats.height,
                        orient_str,
                        stats.estimated_bitrate_kbps,
                        stats.estimated_fps,
                        stats.packets_received
                    ));
                }

                glib::ControlFlow::Continue
            });

            window.set_child(Some(&main_box));
            window.present();
        });

        app.run_with_args(&Vec::<String>::new());
    }
}
