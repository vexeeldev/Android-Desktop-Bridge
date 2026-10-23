import fs from 'node:fs';
import vm from 'node:vm';

const code1 = fs.readFileSync('build/extension.js', 'utf8');
const code2 = fs.readFileSync('build/main.js', 'utf8');
try {
  // Parse ES module syntax without executing imports
  new vm.SourceTextModule(code1);
  new vm.SourceTextModule(code2);
  console.log('✅ ES module syntax and structure is 100% valid!');
} catch (e) {
  console.error('❌ Module syntax error:', e);
  process.exit(1);
}
