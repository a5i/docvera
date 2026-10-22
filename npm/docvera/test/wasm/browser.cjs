'use strict';
// Exercise the actual npm tarball in Chromium, both in-thread and in a module Worker.
const http = require('node:http');
const fs = require('node:fs/promises');
const path = require('node:path');
const { packedPackage } = require('./package-fixture.cjs');
const { chromium } = require('../../../../crates/docvera-wasm/node_modules/playwright');
const { chromiumLaunchOptions } = require('./browser-options.cjs');

const mime = { '.mjs': 'text/javascript', '.js': 'text/javascript', '.wasm': 'application/wasm' };


(async () => {
  const artifact = packedPackage();
  const pageLog = [];
  const manifest = JSON.parse(await fs.readFile(path.join(artifact.installed, 'package.json'), 'utf8'));
  const entry = '/' + manifest.exports['./wasm'].browser.replace(/^\.\//, '');
  const root = artifact.installed;
  const server = http.createServer((req, res) => {
    try {
      const url = new URL(req.url, 'http://localhost').pathname;
      if (url === '/') {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        res.end('<!doctype html><meta charset="utf-8"><title>docvera wasm probe</title>');
        return;
      }
      const file = url === '/test/shared.mjs' ? path.join(__dirname, 'shared.mjs') : path.resolve(root, '.' + url);
      if (url !== '/test/shared.mjs' && !file.startsWith(root + path.sep)) throw new Error('Outside package');
      return fs.readFile(file).then(body => {
        res.writeHead(200, { 'Content-Type': mime[path.extname(file)] || 'application/octet-stream', 'Content-Length': body.length });
        res.end(body);
      }, () => { res.writeHead(404); res.end(); });
    } catch { res.writeHead(404); res.end(); }
  });
  let browser, timer;
  try {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    browser = await chromium.launch(await chromiumLaunchOptions());
    const page = await browser.newPage();
    page.on('console', message => pageLog.push(`${message.type()} ${message.text()}`));
    page.on('pageerror', error => pageLog.push(`pageerror ${error.message}`));
    page.on('crash', () => pageLog.push('PAGE CRASHED'));
    page.on('requestfailed', request => pageLog.push(`requestfailed ${request.url()} ${request.failure()?.errorText ?? ''}`));
    await page.goto(`http://127.0.0.1:${server.address().port}/`, { waitUntil: 'domcontentloaded' });
    await Promise.race([
      page.evaluate(async entry => {
        const wasm = await import(entry);
        const { smoke } = await import('/test/shared.mjs');
        for (const method of ['openChecker', 'openCheckerWorker']) {
          // In particular, this exercises the browser Worker's ready handshake.
          await smoke({ openChecker: wasm[method], trace: stage => console.log(`${method}: ${stage}`) });
        }
      }, entry),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Browser smoke timeout (including Worker startup)')), 60000); }),
    ]).catch(error => { throw new Error(`${error.message}\n${pageLog.join('\n').slice(-4000)}`); });
    console.log('Packed @a5i/docvera/wasm browser API (in-thread + module Worker): PASS');
  } finally {
    clearTimeout(timer);
    await browser?.close();
    if (server.listening) await new Promise(resolve => server.close(resolve));
    artifact.cleanup();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
