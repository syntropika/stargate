'use strict';
const fs = require('node:fs');
const path = require('node:path');

// Source checkouts use a locally built binary; published packages use a platform dependency.
const local = path.join(__dirname, 'stargate.node');
if (fs.existsSync(local)) {
  module.exports = require(local);
} else {
  let target;
  if (process.platform === 'linux' && process.arch === 'x64' &&
      process.report.getReport().header.glibcVersionRuntime) target = 'linux-x64-gnu';
  if (process.platform === 'darwin' && process.arch === 'arm64') target = 'darwin-arm64';
  if (process.platform === 'win32' && process.arch === 'x64') target = 'win32-x64-msvc';
  if (!target) throw new Error(`Stargate has no prebuilt binary for ${process.platform}/${process.arch}. Build from source: https://github.com/syntropika/stargate`);
  try {
    module.exports = require(`@syntropika/stargate-${target}`);
  } catch (cause) {
    throw new Error(`Cannot load Stargate for ${target}. Install optional dependencies and check your Node.js version.`, {cause});
  }
}
