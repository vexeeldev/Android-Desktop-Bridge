#[cfg(feature = "gui")]
pub mod gtk_ui {
    use crate::client::{BridgeClient, ConnectionStatus};
    use crate::identity::IdentityManager;
    use crate::storage::Database;
    use gtk4::glib;
    use gtk4::prelude::*;
    use gtk4::{
        Application, ApplicationWindow, Box, Button, Entry, HeaderBar, Label, Orientation,
    };
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
                .title("Android Desktop Bridge")
                .default_width(700)
                .default_height(480)
                .build();

            let header = HeaderBar::new();
            window.set_titlebar(Some(&header));

            let root_box = Box::new(Orientation::Vertical, 16);
            root_box.set_margin_top(24);
            root_box.set_margin_bottom(24);
            root_box.set_margin_start(24);
            root_box.set_margin_end(24);

            // 1. Status Indicator Banner
            let status_box = Box::new(Orientation::Horizontal, 12);
            status_box.set_halign(gtk4::Align::Center);

            let status_label = Label::new(Some("Connecting to Relay..."));
            status_label.set_css_classes(&["title-3"]);

            let status_badge = Label::new(Some(" ● DISCONNECTED "));
            status_badge.set_css_classes(&["badge", "error"]);
            
            status_box.append(&status_badge);
            status_box.append(&status_label);
            root_box.append(&status_box);

            // CSS Provider for modern Adwaita dark styling & green connected status
            let css = gtk4::CssProvider::new();
            css.load_from_data(
                "
                .badge {
                    padding: 6px 14px;
                    border-radius: 12px;
                    font-weight: bold;
                    font-size: 14px;
                }
                .badge.connected {
                    background-color: #2ec27e;
                    color: #ffffff;
                }
                .badge.waiting {
                    background-color: #f6d32d;
                    color: #1a1a1a;
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
                ",
            );
            gtk4::style_context_add_provider_for_display(
                &gtk4::gdk::Display::default().expect("No display found"),
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            // 2. Identity Info Card
            let identity_card = Box::new(Orientation::Vertical, 8);
            identity_card.set_css_classes(&["card"]);

            let id_title = Label::new(Some("Linux Controller Identity"));
            id_title.set_halign(gtk4::Align::Start);
            id_title.set_css_classes(&["heading"]);

            let id_val = Label::new(Some(&format!("Device ID: {}", identity.device_id())));
            id_val.set_halign(gtk4::Align::Start);
            id_val.set_selectable(true);

            let pubkey_val = Label::new(Some(&format!("Public Key: {}...", &identity.public_key_hex()[..24])));
            pubkey_val.set_halign(gtk4::Align::Start);
            pubkey_val.set_selectable(true);

            identity_card.append(&id_title);
            identity_card.append(&id_val);
            identity_card.append(&pubkey_val);
            root_box.append(&identity_card);

            // 3. Remote Android Device Card
            let device_card = Box::new(Orientation::Vertical, 8);
            device_card.set_css_classes(&["card"]);

            let dev_title = Label::new(Some("Android Device Status"));
            dev_title.set_halign(gtk4::Align::Start);
            dev_title.set_css_classes(&["heading"]);

            let dev_name_label = Label::new(Some("Device: None paired yet"));
            dev_name_label.set_halign(gtk4::Align::Start);

            let dev_id_label = Label::new(Some("Remote ID: -"));
            dev_id_label.set_halign(gtk4::Align::Start);
            dev_id_label.set_selectable(true);

            device_card.append(&dev_title);
            device_card.append(&dev_name_label);
            device_card.append(&dev_id_label);
            root_box.append(&device_card);

            // 4. Pairing Action Card
            let pair_card = Box::new(Orientation::Vertical, 8);
            pair_card.set_css_classes(&["card"]);

            let pair_title = Label::new(Some("Pairing"));
            pair_title.set_halign(gtk4::Align::Start);
            pair_title.set_css_classes(&["heading"]);

            let input_box = Box::new(Orientation::Horizontal, 12);
            let pin_entry = Entry::new();
            pin_entry.set_placeholder_text(Some("Enter 6-digit Android PIN"));

            let pair_btn = Button::with_label("Pair with Android");
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
            root_box.append(&pair_card);

            // Initial state check
            if let Ok(trusted) = db.list_trusted_devices() {
                if let Some(first) = trusted.first() {
                    dev_name_label.set_text(&format!("Device: {}", first.name));
                    dev_id_label.set_text(&format!("Remote ID: {}", first.device_id));
                }
            }

            // Periodic UI Poller to update connection status
            let mut status_rx = client.status_receiver();
            let status_label_clone = status_label.clone();
            let status_badge_clone = status_badge.clone();
            let dev_name_clone = dev_name_label.clone();
            let dev_id_clone = dev_id_label.clone();

            glib::timeout_add_local(Duration::from_millis(200), move || {
                let current = status_rx.borrow_and_update().clone();
                match current {
                    ConnectionStatus::Disconnected => {
                        status_badge_clone.set_text(" ● DISCONNECTED ");
                        status_badge_clone.set_css_classes(&["badge", "error"]);
                        status_label_clone.set_text("Relay disconnected");
                    }
                    ConnectionStatus::ConnectingToRelay => {
                        status_badge_clone.set_text(" ◌ CONNECTING ");
                        status_badge_clone.set_css_classes(&["badge", "waiting"]);
                        status_label_clone.set_text("Connecting to relay server...");
                    }
                    ConnectionStatus::RelayConnectedWaitingForAndroid => {
                        status_badge_clone.set_text(" ◌ WAITING ");
                        status_badge_clone.set_css_classes(&["badge", "waiting"]);
                        status_label_clone.set_text("Relay Connected — Waiting for Android");
                    }
                    ConnectionStatus::PairingInProgress => {
                        status_badge_clone.set_text(" ◌ PAIRING ");
                        status_badge_clone.set_css_classes(&["badge", "waiting"]);
                        status_label_clone.set_text("Pairing in progress with Android agent...");
                    }
                    ConnectionStatus::AndroidConnected { device_id, device_name } => {
                        // SPEC REQUIREMENT #9: Tampilkan status "Android Connected" di Linux setelah pairing berhasil
                        status_badge_clone.set_text(" ● Android Connected ");
                        status_badge_clone.set_css_classes(&["badge", "connected"]);
                        status_label_clone.set_text(&format!("Connected: {}", device_name));
                        dev_name_clone.set_text(&format!("Device: {}", device_name));
                        dev_id_clone.set_text(&format!("Remote ID: {}", device_id));
                    }
                    ConnectionStatus::Error(msg) => {
                        status_badge_clone.set_text(" ● ERROR ");
                        status_badge_clone.set_css_classes(&["badge", "error"]);
                        status_label_clone.set_text(&msg);
                    }
                }
                glib::ControlFlow::Continue
            });

            window.set_child(Some(&root_box));
            window.present();
        });

        app.run_with_args(&Vec::<String>::new());
    }
}
