'use strict';
const assert = require('node:assert/strict');

function lineEndingsLike(source, text) {
  const newline = source.includes('\r\n') ? '\r\n' : '\n';
  return text.replace(/\r\n/g, '\n').replace(/\n/g, newline);
}

// Adapt the mutation templates, never normalize the entire source document.
function replaceFixtureText(source, from, to) {
  from = lineEndingsLike(source, from);
  to = lineEndingsLike(source, to);
  assert.ok(source.includes(from), `Missing fixture text: ${JSON.stringify(from)}`);
  return source.replace(from, () => to);
}

module.exports = { replaceFixtureText };
