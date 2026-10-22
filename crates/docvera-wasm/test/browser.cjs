// No browser automation dependency: serve ES modules/WASM and inspect headless Chromium's DOM.
const http = require('node:http');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { spawn } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm' };
const server = http.createServer(async (req, res) => {
  try {
    const file = path.resolve(root, '.' + new URL(req.url, 'http://localhost').pathname);
    if (!file.startsWith(root + path.sep)) throw new Error('Outside root');
    const body = await fs.readFile(file);
    res.writeHead(200, { 'Content-Type': mime[path.extname(file)] || 'application/octet-stream' });
    res.end(body);
  } catch { res.writeHead(404); res.end(); }
});
(async () => {
  const profile = await fs.mkdtemp(path.join(os.tmpdir(), 'docvera-browser-'));
  try {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const url = `http://127.0.0.1:${server.address().port}/test/browser.html`;
    const child = spawn(process.env.CHROMIUM || 'chromium', [
      '--headless', '--disable-gpu', '--no-first-run', `--user-data-dir=${profile}`,
      '--virtual-time-budget=15000', '--dump-dom', url
    ]);
    let stdout = '', stderr = '';
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    const timer = setTimeout(() => child.kill('SIGKILL'), 60000);
    try {
      const code = await new Promise((resolve, reject) => {
        child.on('error', reject); child.on('close', resolve);
      });
      if (code !== 0 || !stdout.includes('<pre id="result">PASS</pre>')) {
        throw new Error(`Browser smoke failed (exit ${code})\n${stdout}\n${stderr}`);
      }
      console.log('Browser WASM smoke: PASS');
    } finally { clearTimeout(timer); }
  } finally {
    await new Promise(resolve => server.close(resolve));
    await fs.rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
