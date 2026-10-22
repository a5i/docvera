'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, writeFileSync, chmodSync, mkdirSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { chromiumLaunchOptions } = require('./browser-options.cjs');

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'docvera-browser-options-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const executable = join(root, 'chromium');
  writeFileSync(executable, '#!/bin/sh\nexit 0\n');
  chmodSync(executable, 0o755);
  return { root, executable };
}

test('default browser is Playwright-managed even when a system Chromium exists on PATH', async t => {
  const { root } = fixture(t);
  assert.deepEqual(await chromiumLaunchOptions({ PATH: root }), { headless: true });
});

test('explicit CHROMIUM supports absolute paths and command names', async t => {
  const { root, executable } = fixture(t);
  const expected = { headless: true, executablePath: executable };
  assert.deepEqual(await chromiumLaunchOptions({ CHROMIUM: executable }), expected);
  assert.deepEqual(await chromiumLaunchOptions({ CHROMIUM: 'chromium', PATH: root }), expected);
});

test('invalid explicit CHROMIUM fails instead of falling back to another browser', async t => {
  const { root } = fixture(t);
  await assert.rejects(chromiumLaunchOptions({ CHROMIUM: 'missing-browser', PATH: root }), /Chromium executable not found/);
  const directory = join(root, 'not-a-browser');
  mkdirSync(directory);
  await assert.rejects(chromiumLaunchOptions({ CHROMIUM: directory }), /Chromium executable not found/);
});
