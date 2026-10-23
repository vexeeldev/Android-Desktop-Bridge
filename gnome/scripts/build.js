import * as esbuild from 'esbuild';
import fs from 'node:fs';
import path from 'node:path';

async function build() {
  console.log('🚀 Building Bridge GNOME Extension...');

  // Ensure build directory exists
  if (!fs.existsSync('build')) {
    fs.mkdirSync('build', { recursive: true });
  }

  // Bundle TypeScript to ES Module for GJS (main.js)
  await esbuild.build({
    entryPoints: ['src/extension.ts'],
    outfile: 'build/main.js',
    bundle: true,
    format: 'esm',
    target: 'es2022',
    external: ['gi://*', 'resource://*'],
    sourcemap: false,
  });

  // Create lightweight cache-busting loader as extension.js
  const loaderCode = `import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

export default class BridgeExtensionLoader extends Extension {
  async enable() {
    try {
      const uri = \`\${this.dir.get_uri()}/main.js?v=\${Date.now()}\`;
      const mod = await import(uri);
      this._app = new mod.default(this);
      this._app.enable();
    } catch (err) {
      console.error('[Bridge GNOME] Error during dynamic load:', err);
    }
  }

  disable() {
    if (this._app) {
      try {
        this._app.disable();
      } catch (err) {
        console.error('[Bridge GNOME] Error during disable:', err);
      }
      this._app = null;
    }
  }
}
`;
  fs.writeFileSync('build/extension.js', loaderCode);

  // Copy metadata.json
  fs.copyFileSync('metadata.json', 'build/metadata.json');

  // Copy stylesheet.css to root of build and to styles/ for GNOME Shell compatibility
  if (fs.existsSync('styles/stylesheet.css')) {
    fs.copyFileSync('styles/stylesheet.css', 'build/stylesheet.css');
    fs.mkdirSync('build/styles', { recursive: true });
    fs.copyFileSync('styles/stylesheet.css', 'build/styles/stylesheet.css');
  }

  // Copy public directory and assets
  if (fs.existsSync('public')) {
    fs.cpSync('public', 'build/public', { recursive: true });
    if (fs.existsSync('public/whatsapp.webp')) {
      fs.copyFileSync('public/whatsapp.webp', 'build/whatsapp.webp');
      fs.mkdirSync('build/styles', { recursive: true });
      fs.copyFileSync('public/whatsapp.webp', 'build/styles/whatsapp.webp');
      fs.mkdirSync('build/styles/public', { recursive: true });
      fs.copyFileSync('public/whatsapp.webp', 'build/styles/public/whatsapp.webp');
    }
  }

  // Pack extension bundle
  try {
    const { execSync } = await import('node:child_process');
    execSync('gnome-extensions pack build --force --out-dir=build', { stdio: 'pipe' });
    console.log('📦 Created shell-extension.zip bundle in build/');
  } catch (err) {
    // Optional if gnome-extensions CLI is not available in some environments
  }

  console.log('✅ Build completed successfully into build/');
}

build().catch((err) => {
  console.error('❌ Build failed:', err);
  process.exit(1);
});

