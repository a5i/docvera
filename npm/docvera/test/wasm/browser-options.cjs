'use strict';
const fs = require('node:fs/promises');
const { constants } = require('node:fs');
const path = require('node:path');

async function chromiumLaunchOptions(env = process.env) {
  // Playwright chooses the matching managed Chromium by default. Do not search
  // PATH unless the caller explicitly opts into a system browser via CHROMIUM.
  if (!env.CHROMIUM) return { headless: true };
  const candidate = env.CHROMIUM;
  const paths = path.isAbsolute(candidate) ? [candidate]
    : (env.PATH || '').split(path.delimiter).map(dir => path.resolve(dir, candidate));
  for (const executablePath of paths) {
    try {
      await fs.access(executablePath, constants.X_OK);
      if ((await fs.stat(executablePath)).isFile()) return { headless: true, executablePath };
    } catch { /* try next PATH entry */ }
  }
  throw new Error(`Chromium executable not found: ${candidate}`);
}

module.exports = { chromiumLaunchOptions };
