'use strict';

// Release jobs merge uniquely named binaries into ONE npm package.
const assert = require('node:assert/strict');
const { readFileSync, readdirSync, statSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { binaryNames } = require('../platforms.cjs');

function validateRelease(directory) {
  const actual = readdirSync(directory).filter(name => /^docvera\..*\.node$/.test(name)).sort();
  assert.deepEqual(actual, [...binaryNames].sort(), 'Expected all five platform binaries in one package');
  const manifest = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'));
  assert.equal(manifest.name, '@a5i/docvera');
  assert.ok(!manifest.optionalDependencies, 'Platform packages are not used');
  for (const filename of [...binaryNames, 'index.js', 'index.d.ts', 'platforms.cjs', 'LICENSE']) {
    const file = statSync(join(directory, filename));
    assert.ok(file.isFile() && file.size > 0, `Missing or empty release file: ${filename}`);
  }
}

if (require.main === module) {
  validateRelease(resolve(process.argv[2] || join(__dirname, '..')));
  console.log('Single @a5i/docvera package contains all five platform binaries.');
}

module.exports = { validateRelease };
