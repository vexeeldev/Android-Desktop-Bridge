# Bridge GNOME

Prototype UI untuk **GNOME Shell Extension** yang menjadi visual frontend dari **Android Desktop Bridge**.

Proyek ini dibangun menggunakan **TypeScript** murni yang ditranspilasi ke **GJS (GNOME JavaScript / ES Modules)** untuk runtime native **GNOME Shell 46**, dengan visual dan layout yang presisi mengacu pada desain referensi Figma.

---

## 🎨 UI & Desain Visual (Berdasarkan Referensi Figma)

Extension ini beroperasi sepenuhnya di dalam **GNOME Shell** (tanpa main window desktop application ataupun web view/browser):

1. **Top Bar Indicator**:
   - Menampilkan icon status (`phonelink-symbolic`).
   - Badge counter hijau dinamis untuk jumlah event yang belum dibaca (`unread`).
2. **Unified Activity Hub (Dual Pane Master-Detail)**:
   - **Header**:
     - Status koneksi dengan indikator dot hijau: `• V211 Connected`.
     - Tombol `Simulasi +` untuk memicu test custom notification popup.
     - Icon Settings (`⚙`).
   - **Left Pane (Activity List)**:
     - Daftar notifikasi dan aktivitas terpadu (WhatsApp, Telepon Masuk, Tugas Sekolah, Gmail, Sistem/Baterai).
     - Menampilkan icon aplikasi spesifik, pengirim/judul, cuplikan pesan, status badge `!`, serta link aksi cepat `Mark as read`.
     - Highlight aktif saat item dipilih.
   - **Right Pane (Detail View)**:
     - **Message Detail**: Header pengirim, riwayat pesan obrolan (bubble pesan masuk & pesan keluar berlatar hijau khas Android/WhatsApp), serta kapsul input `Start message` dengan tombol kirim bulat hijau `➤`. Membalas pesan langsung memperbarui percakapan secara interaktif.
     - **Call Detail**: Avatar telepon besar 📞, nama pemanggil, dan tombol aksi `[ Decline ]` serta `[ Answer ]`.
     - **Generic/School/Email Detail**: Rincian dokumen/tugas dengan aksi konfirmasi selesai atau dismiss.
3. **Custom Notification Popup**:
   - Banner mengambang (*floating pill*) yang muncul dari area top bar saat event baru diterima.
   - Menggunakan St actor custom (bukan sekadar notifikasi default OS).
   - Menampilkan icon, nama aplikasi, judul, cuplikan isi pesan, dan tombol dismiss.
   - Mengklik banner akan langsung membuka Activity Hub dan menyorot percakapan tersebut.
   - Dilengkapi animasi fade-in dan auto-hide setelah 5 detik.

---

## 🏗️ Arsitektur

Struktur arsitektur didesain modular dan decoupled, sehingga sumber event (*Activity Provider*) dapat dengan mudah diganti dari mock data ke Linux Daemon (Bridge Linux daemon via D-Bus atau WebSocket) tanpa mengubah komponen UI sama sekali:

```text
[ MockActivityProvider / BridgeDaemonProvider ]
                     │
                     ▼
             [ ActivityStore ] (Observable State)
              ├── getEvents()
              ├── unreadCount
              ├── sendReply()
              └── activeItem
                     │
       ┌─────────────┴─────────────┐
       ▼                           ▼
[ Top Bar Indicator ]   [ NotificationPopupManager ]
       │                           │
       ▼                           │
[ ActivityHubWidget ]              │
  ├── Header                       │
  ├── ActivityItemWidget (List)    │
  └── DetailViewWidget (Detail) ◄──┘
```

### Komponen Utama:
- [src/types.ts](file:///home/malakul-tech/project/Bridge/gnome/src/types.ts): Definisi kontrak tipe data event (`ActivityEvent`, `ActivityType`, `ChatMessage`), interface `IActivityStore`, dan `IActivityProvider`.
- [src/activityItem.ts](file:///home/malakul-tech/project/Bridge/gnome/src/activityItem.ts): Widget item kartu daftar aktivitas untuk unified hub list.
- [src/detailView.ts](file:///home/malakul-tech/project/Bridge/gnome/src/detailView.ts): Widget panel detail (chat bubbles, input bar, call actions).
- [src/activityHub.ts](file:///home/malakul-tech/project/Bridge/gnome/src/activityHub.ts): Implementasi `ActivityStore`, `MockActivityProvider`, dan layout master-detail `ActivityHubWidget`.
- [src/notificationPopup.ts](file:///home/malakul-tech/project/Bridge/gnome/src/notificationPopup.ts): Manager popup mengambang top bar dengan auto-hide dan animasi Clutter.
- [src/indicator.ts](file:///home/malakul-tech/project/Bridge/gnome/src/indicator.ts): Subclass `PanelMenu.Button` untuk GNOME status area.
- [src/extension.ts](file:///home/malakul-tech/project/Bridge/gnome/src/extension.ts): Entry point extension dan lifecycle manager (`enable`, `disable`).
- [styles/stylesheet.css](file:///home/malakul-tech/project/Bridge/gnome/styles/stylesheet.css): Styling lengkap bernuansa dark modern glassmorphism sesuai Figma.

---

## 📁 Struktur Direktori

```text
Bridge/gnome/
├── src/
│   ├── ambient.d.ts           # Ambient type definitions untuk GJS & GNOME 46
│   ├── extension.ts          # Entry point GNOME Shell Extension
│   ├── indicator.ts          # Top bar button & indicator
│   ├── activityHub.ts        # Unified activity store, provider & hub UI
│   ├── notificationPopup.ts  # Custom floating notification popup
│   ├── activityItem.ts       # Activity list item widget
│   ├── detailView.ts         # Detail view (chat bubbles & call view)
│   └── types.ts              # Interface & data types
├── styles/
│   └── stylesheet.css        # Styling UI Shell (dark glassmorphism)
├── scripts/
│   ├── build.js              # Build script (esbuild bundler & pack)
│   ├── install.js            # Installer ke ~/.local/share/gnome-shell/extensions
│   └── test-syntax.js        # Validasi sintaks modul ES
├── metadata.json             # Manifest GNOME Shell extension
├── package.json              # Script npm & dependencies
├── tsconfig.json             # Konfigurasi TypeScript
├── README.md                 # Dokumentasi project
└── build/                    # Hasil transpilasi (siap pasang di GNOME)
    ├── extension.js
    ├── metadata.json
    ├── stylesheet.css
    ├── styles/
    └── bridge-gnome@bridge.org.shell-extension.zip
```

---

## 🛠️ Instalasi & Pengembangan

### 1. Prasyarat
- GNOME Shell 46
- Node.js (v18+) dan npm

### 2. Kompilasi & Build
Jalankan perintah build untuk mentranspile TypeScript ke bundel ES module GJS:
```bash
npm run build
```

Untuk memvalidasi TypeScript types, build, dan integritas modul sekaligus:
```bash
npm run validate
```

### 3. Pasang ke GNOME Shell
Salin bundel hasil build ke direktori ekstensi pengguna:
```bash
npm run install-extension
```

### 4. Menjalankan / Mengaktifkan Ekstensi di GNOME

#### Di Wayland Session:
Karena GNOME Shell di sesi Wayland memuat daftar ekstensi baru saat inisialisasi sesi:
1. Pasang ekstensi dengan `npm run install-extension`.
2. Lakukan **Log Out** dan **Log In** kembali ke sesi GNOME (atau gunakan nested session untuk pengujian: `gnome-shell --nested --wayland`).
3. Aktifkan ekstensi:
   ```bash
   gnome-extensions enable bridge-gnome@bridge.org
   ```
4. Atau buka aplikasi **Extensions** / **Extension Manager** dan aktifkan toggle **Bridge GNOME**.

#### Di X11 Session:
1. Tekan `Alt` + `F2`, ketik `r`, lalu tekan `Enter` untuk merestart GNOME Shell tanpa keluar sesi.
2. Aktifkan ekstensi:
   ```bash
   gnome-extensions enable bridge-gnome@bridge.org
   ```

### 5. Memeriksa Log
Untuk memantau log aktivitas ekstensi:
```bash
journalctl -f -o cat /usr/bin/gnome-shell | grep -i "Bridge GNOME"
```
