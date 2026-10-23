import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

function install() {
  const metadata = JSON.parse(fs.readFileSync('metadata.json', 'utf8'));
  const uuid = metadata.uuid;
  const targetDir = path.join(os.homedir(), '.local', 'share', 'gnome-shell', 'extensions', uuid);

  console.log(`📦 Installing extension ${uuid} to: ${targetDir}`);

  if (!fs.existsSync(targetDir)) {
    fs.mkdirSync(targetDir, { recursive: true });
  }

  // Copy all files from build/ to targetDir
  function copyRecursive(src, dest) {
    if (fs.statSync(src).isDirectory()) {
      if (!fs.existsSync(dest)) fs.mkdirSync(dest, { recursive: true });
      for (const child of fs.readdirSync(src)) {
        copyRecursive(path.join(src, child), path.join(dest, child));
      }
    } else {
      fs.copyFileSync(src, dest);
    }
  }

  copyRecursive('build', targetDir);
  console.log(`✅ Extension installed successfully to ${targetDir}`);
}

install();
