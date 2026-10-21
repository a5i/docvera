'use strict';

// One npm package contains every supported platform binary, as in eregex.
const PLATFORMS = {
  'linux-x64': 'linux-x64-gnu',
  'linux-arm64': 'linux-arm64-gnu',
  'darwin-x64': 'darwin-x64',
  'darwin-arm64': 'darwin-arm64',
  'win32-x64': 'win32-x64-msvc',
};

function platformName(os = process.platform, arch = process.arch) {
  const name = PLATFORMS[`${os}-${arch}`];
  if (!name) throw new Error(`Unsupported docvera platform: ${os}-${arch}`);
  return name;
}

const binaryNames = Object.values(PLATFORMS).map(name => `docvera.${name}.node`);
module.exports = { platformName, binaryNames };
