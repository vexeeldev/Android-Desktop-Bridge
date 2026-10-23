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
        MenuButton, Orientation, Popover, ScrolledWindow,
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
        let app = Application::builder().application_id(app_id).build();

        app.connect_activate(move |app| {
            let window = ApplicationWindow::builder()
                .application(app)
                .title("Android Desktop Bridge")
                .default_width(1280)
                .default_height(760)
                .build();

            let header = HeaderBar::new();
            window.set_titlebar(Some(&header));

            // Modern Dark CSS styling
            let css = gtk4::CssProvider::new();
            css.load_from_data(
                "
                .badge {
                    padding: 4px 10px;
                    border-radius: 10px;
                    font-weight: bold;
                    font-size: 12px;
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
                    padding: 14px;
                }
                .dim-label {
                    opacity: 0.7;
                    font-weight: 500;
                }
                .stats-label {
                    font-family: monospace;
                    font-size: 12px;
                    opacity: 0.9;
                }
                ",
            );
            gtk4::style_context_add_provider_for_display(
                &gtk4::gdk::Display::default().expect("No display found"),
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // ============================================================
            // HEADER BAR: LEFT SIDE (STATUS & METRICS)
            // ============================================================
            let header_left = Box::new(Orientation::Horizontal, 10);
            header_left.set_margin_start(6);

            let app_title = Label::new(Some("Android Bridge"));
            app_title.set_css_classes(&["title-4"]);

            let webrtc_badge = Label::new(Some(" ● WebRTC Disconnected "));
            webrtc_badge.set_css_classes(&["badge", "error"]);

            let metrics_label = Label::new(Some("Waiting for stream..."));
            metrics_label.set_css_classes(&["stats-label"]);

            header_left.append(&app_title);
            header_left.append(&webrtc_badge);
            header_left.append(&metrics_label);
            header.pack_start(&header_left);

            // ============================================================
            // HEADER BAR: RIGHT SIDE (ACTIONS & POPUP MENU)
            // ============================================================
            let header_right = Box::new(Orientation::Horizontal, 8);
            header_right.set_margin_end(6);

            let btn_stream_toggle = Button::with_label("▶ Start Stream");
            btn_stream_toggle.set_css_classes(&["suggested-action"]);

            // Mode Aspect: 0 (Fit Rasio Asli), 1 (Stretch Fill Layar Penuh)
            let stretch_mode = Rc::new(RefCell::new(false));
            let btn_aspect = Button::with_label("📐 Mode: Fit Rasio");

            let btn_fullscreen = Button::with_label("⛶ Fullscreen");

            // Floating Popup Menu Button
            let menu_btn = MenuButton::new();
            menu_btn.set_label("📱 Device & Pairing");

            header_right.append(&btn_stream_toggle);
            header_right.append(&btn_aspect);
            header_right.append(&menu_btn);
            header_right.append(&btn_fullscreen);
            header.pack_end(&header_right);

            // ============================================================
            // POPOVER CONTENT: SPECIFICATIONS, PAIRING & IDENTITY
            // ============================================================
            let popover = Popover::new();
            let popover_scroll = ScrolledWindow::new();
            popover_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
            popover_scroll.set_min_content_width(380);
            popover_scroll.set_max_content_height(560);

            let popover_box = Box::new(Orientation::Vertical, 12);
            popover_box.set_margin_top(14);
            popover_box.set_margin_bottom(14);
            popover_box.set_margin_start(14);
            popover_box.set_margin_end(14);

            // 1. Status Indicator Banner
            let status_box = Box::new(Orientation::Horizontal, 10);
            let status_badge = Label::new(Some(" ● DISCONNECTED "));
            status_badge.set_css_classes(&["badge", "error"]);
            let status_label = Label::new(Some("Relay disconnected"));
            status_label.set_css_classes(&["title-4"]);
            status_box.append(&status_badge);
            status_box.append(&status_label);
            popover_box.append(&status_box);

            // 2. Android Device Information Panel
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
            popover_box.append(&device_card);

            // 3. Linux Controller Identity Card
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
            popover_box.append(&identity_card);

            // 4. Pairing Card
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
            popover_box.append(&pair_card);

            // 5. Reconnect Button
            let btn_reconnect = Button::with_label("↻ Reconnect P2P Stream");
            popover_box.append(&btn_reconnect);

            popover_scroll.set_child(Some(&popover_box));
            popover.set_child(Some(&popover_scroll));
            menu_btn.set_popover(Some(&popover));

            // ============================================================
            // FULL-WINDOW SCREEN STREAMING VIEWPORT (NO BOX CROWDING)
            // ============================================================
            let current_surface: Rc<RefCell<Option<cairo::ImageSurface>>> =
                Rc::new(RefCell::new(None));
            let drawing_area = DrawingArea::new();
            drawing_area.set_hexpand(true);
            drawing_area.set_vexpand(true);

            let surface_ref = Rc::clone(&current_surface);
            let is_stretch = Rc::clone(&stretch_mode);
            drawing_area.set_draw_func(move |_, cr, width, height| {
                let stretch = *is_stretch.borrow();
                if let Some(surface) = surface_ref.borrow().as_ref() {
                    let frame_w = surface.width() as f64;
                    let frame_h = surface.height() as f64;
                    let target_w = width as f64;
                    let target_h = height as f64;

                    let _ = cr.save();
                    // Background
                    cr.set_source_rgb(0.04, 0.06, 0.09);
                    let _ = cr.paint();

                    if stretch {
                        // Stretch to fill 100% window dimensions
                        cr.scale(target_w / frame_w, target_h / frame_h);
                        let _ = cr.set_source_surface(surface, 0.0, 0.0);
                        let _ = cr.paint();
                    } else {
                        // Proportional aspect-ratio fit
                        let scale = (target_w / frame_w).min(target_h / frame_h);
                        let draw_w = frame_w * scale;
                        let draw_h = frame_h * scale;
                        let offset_x = (target_w - draw_w) / 2.0;
                        let offset_y = (target_h - draw_h) / 2.0;

                        cr.translate(offset_x, offset_y);
                        cr.scale(scale, scale);
                        let _ = cr.set_source_surface(surface, 0.0, 0.0);
                        let _ = cr.paint();
                    }

                    let _ = cr.restore();
                } else {
                    // Sleek dark placeholder when waiting for stream
                    cr.set_source_rgb(0.04, 0.06, 0.09);
                    let _ = cr.paint();

                    cr.set_source_rgb(0.85, 0.9, 0.98);
                    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    cr.set_font_size(22.0);
                    let text = "Android Desktop Bridge";
                    if let Ok(extents) = cr.text_extents(text) {
                        cr.move_to(
                            (width as f64 - extents.width()) / 2.0,
                            height as f64 / 2.0 - 15.0,
                        );
                        let _ = cr.show_text(text);
                    }

                    cr.select_font_face(
                        "Sans",
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Normal,
                    );
                    cr.set_font_size(14.0);
                    cr.set_source_rgba(0.7, 0.75, 0.85, 0.75);
                    let subtext = "Full Window Screen Mirroring • Press ▶ Start Stream to connect";
                    if let Ok(sub_ext) = cr.text_extents(subtext) {
                        cr.move_to(
                            (width as f64 - sub_ext.width()) / 2.0,
                            height as f64 / 2.0 + 20.0,
                        );
                        let _ = cr.show_text(subtext);
                    }
                }
            });

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

            // Connect Aspect / Stretch mode button
            let stretch_click = Rc::clone(&stretch_mode);
            let btn_aspect_clone = btn_aspect.clone();
            let drawing_area_aspect = drawing_area.clone();
            btn_aspect.connect_clicked(move |_| {
                let current = *stretch_click.borrow();
                let new_mode = !current;
                *stretch_click.borrow_mut() = new_mode;
                if new_mode {
                    btn_aspect_clone.set_label("↔️ Mode: Stretch (Penuh)");
                } else {
                    btn_aspect_clone.set_label("📐 Mode: Fit Rasio");
                }
                drawing_area_aspect.queue_draw();
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
            let mut webrtc_frame_rx = webrtc_rec.frame_receiver();

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
            let current_surface_cleaner = Rc::clone(&current_surface);
            let drawing_area_cleaner = drawing_area.clone();

            // High frequency (approx 60 FPS / 16ms) video frame update loop
            let current_surface_updater = Rc::clone(&current_surface);
            let drawing_area_redraw = drawing_area.clone();
            let mut frame_render_count = 0u64;
            let mut last_render_log = std::time::Instant::now();

            glib::timeout_add_local(Duration::from_millis(16), move || {
                if webrtc_frame_rx.has_changed().unwrap_or(false) {
                    if let Some(frame) = webrtc_frame_rx.borrow_and_update().clone() {
                        let w = frame.width as i32;
                        let h = frame.height as i32;
                        let mut recreate = match current_surface_updater.borrow().as_ref() {
                            Some(s) => s.width() != w || s.height() != h,
                            None => true,
                        };

                        if !recreate {
                            if let Some(surf) = current_surface_updater.borrow_mut().as_mut() {
                                if let Ok(mut data) = surf.data() {
                                    let len = data.len().min(frame.bgra_data.len());
                                    data[..len].copy_from_slice(&frame.bgra_data[..len]);
                                } else {
                                    recreate = true;
                                }
                            }
                        }

                        if recreate {
                            if let Ok(mut surf) =
                                cairo::ImageSurface::create(cairo::Format::Rgb24, w, h)
                            {
                                if let Ok(mut data) = surf.data() {
                                    let len = data.len().min(frame.bgra_data.len());
                                    data[..len].copy_from_slice(&frame.bgra_data[..len]);
                                }
                                *current_surface_updater.borrow_mut() = Some(surf);
                            }
                        }

                        drawing_area_redraw.queue_draw();

                        frame_render_count += 1;
                        if frame_render_count <= 5
                            || last_render_log.elapsed() >= Duration::from_secs(2)
                        {
                            tracing::info!(
                                "GTK4 renderer drew frame #{}: {}x{} (full-window redrawn)",
                                frame_render_count,
                                w,
                                h
                            );
                            last_render_log = std::time::Instant::now();
                        }
                    }
                }
                glib::ControlFlow::Continue
            });

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
                    ConnectionStatus::AndroidConnected {
                        device_id,
                        device_name,
                    } => {
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
                    dev_os_c.set_text(&format!(
                        "Android {} (API {})",
                        info.android_version, info.sdk_level
                    ));
                    let charging_str = if info.is_charging {
                        "⚡ Charging"
                    } else {
                        "Discharging"
                    };
                    dev_battery_c.set_text(&format!("{}% ({})", info.battery_level, charging_str));
                    dev_screen_c.set_text(&format!(
                        "{} × {} ({})",
                        info.screen_width, info.screen_height, info.orientation
                    ));
                    dev_service_c.set_text(&info.service_status);
                    dev_id_c.set_text(&info.device_id);
                }

                // 3. WebRTC Stream State Indicator update
                let webrtc_state = webrtc_state_rx.borrow_and_update().clone();
                match webrtc_state {
                    WebRtcState::Disconnected => {
                        webrtc_badge_c.set_text(" ● WebRTC: Disconnected ");
                        webrtc_badge_c.set_css_classes(&["badge", "error"]);
                        btn_stream_toggle_c.set_label("▶ Start Stream");
                        *is_streaming_c.borrow_mut() = false;
                        if current_surface_cleaner.borrow().is_some() {
                            *current_surface_cleaner.borrow_mut() = None;
                            drawing_area_cleaner.queue_draw();
                        }
                    }
                    WebRtcState::Connecting => {
                        webrtc_badge_c.set_text(" ◌ WebRTC: Connecting... ");
                        webrtc_badge_c.set_css_classes(&["badge", "waiting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::ConnectedDirectP2P => {
                        webrtc_badge_c.set_text(" ● P2P Connected ");
                        webrtc_badge_c.set_css_classes(&["badge", "connected"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::ConnectedRelayFallback => {
                        webrtc_badge_c.set_text(" ● Relay Fallback ");
                        webrtc_badge_c.set_css_classes(&["badge", "waiting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::Reconnecting => {
                        webrtc_badge_c.set_text(" ◌ Reconnecting... ");
                        webrtc_badge_c.set_css_classes(&["badge", "reconnecting"]);
                        btn_stream_toggle_c.set_label("⏹ Stop Stream");
                        *is_streaming_c.borrow_mut() = true;
                    }
                    WebRtcState::Failed(msg) => {
                        webrtc_badge_c.set_text(&format!(" ● WebRTC Failed: {} ", msg));
                        webrtc_badge_c.set_css_classes(&["badge", "error"]);
                        btn_stream_toggle_c.set_label("▶ Start Stream");
                        *is_streaming_c.borrow_mut() = false;
                    }
                }

                // 4. Adaptive Bitrate & Stream Metrics update
                let stats = webrtc_stats_rx.borrow_and_update().clone();
                if *is_streaming_c.borrow() {
                    let orient_str = if stats.is_landscape {
                        "Landscape Asli"
                    } else {
                        "Portrait (Miringkan HP untuk Landscape Penuh)"
                    };
                    metrics_label_c.set_text(&format!(
                        "{}×{} [{}] | Low Latency | {} kbps | {} FPS",
                        stats.width,
                        stats.height,
                        orient_str,
                        stats.estimated_bitrate_kbps,
                        stats.estimated_fps
                    ));
                }

                glib::ControlFlow::Continue
            });

            // Set DrawingArea as full-window content
            window.set_child(Some(&drawing_area));
            window.present();
        });

        app.run_with_args(&Vec::<String>::new());
    }
}
