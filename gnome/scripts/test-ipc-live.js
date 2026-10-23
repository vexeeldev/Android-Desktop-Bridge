import net from 'net';
import fs from 'fs';

const sockPath = `${process.env.XDG_RUNTIME_DIR || '/tmp'}/bridge-daemon.sock`;

if (fs.existsSync(sockPath)) {
  fs.unlinkSync(sockPath);
}

console.log(`[Test IPC] Starting mock daemon IPC server on: ${sockPath}`);

const server = net.createServer((socket) => {
  console.log('[Test IPC] ✅ GNOME Extension successfully connected to IPC socket!');

  // 1. Send initial DeviceStatus
  const deviceStatus = JSON.stringify({
    type: 'DeviceStatus',
    data: {
      connected: true,
      device_name: 'Google Pixel 8 (Live Tailscale)',
      device_id: 'android-pixel8-tailscale',
      battery_level: 88,
      is_charging: true,
    },
  }) + '\n';

  socket.write(deviceStatus);
  console.log('[Test IPC] Sent DeviceStatus: Google Pixel 8 (Live Tailscale)');

  // 2. Send Real WhatsApp notification after 1.5 seconds
  setTimeout(() => {
    const waEvent = JSON.stringify({
      type: 'ActivityEvent',
      data: {
        event_id: 'wa-msg-live-101',
        category: 'MESSAGE',
        package_name: 'com.whatsapp',
        app_name: 'WhatsApp',
        title: 'Budi',
        body: 'ke kantor jam berapa?',
        timestamp: Date.now(),
        unread: true,
      },
    }) + '\n';

    socket.write(waEvent);
    console.log('[Test IPC] Sent ActivityEvent: WhatsApp | Budi: ke kantor jam berapa?');
  }, 1500);

  // 3. Keep open for a few seconds then cleanly close
  setTimeout(() => {
    console.log('[Test IPC] Test completed successfully. Closing server.');
    socket.end();
    server.close();
  }, 6000);
});

server.listen(sockPath, () => {
  console.log('[Test IPC] Waiting for GNOME Extension to connect (within 3 seconds reconnect window)...');
});
