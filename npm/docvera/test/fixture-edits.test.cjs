'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { replaceFixtureText } = require('./support/fixture-text.cjs');

for (const newline of ['\n', '\r\n']) {
  test(`fixture mutations preserve ${JSON.stringify(newline)}, BOM and unrelated bytes`, () => {
    const prefix = `\uFEFF---${newline}id: FEATURE-001${newline}`;
    const suffix = `---${newline}# Résumé${newline}Body.  ${newline}`;
    const source = `${prefix}owner: identity-team${newline}${suffix}`;
    assert.equal(replaceFixtureText(source, 'owner: identity-team\n', ''), `${prefix}${suffix}`);
    assert.equal(
      replaceFixtureText(source, 'owner: identity-team\r\n', 'owner: platform-team\nstatus: planned\n'),
      `${prefix}owner: platform-team${newline}status: planned${newline}${suffix}`,
    );
    // Strings used by the actual operational/architecture scenario mutations.
    assert.equal(replaceFixtureText(`runbook: RUN-001${newline}`, 'runbook: RUN-001\n', ''), '');
    assert.equal(replaceFixtureText(`components: [COMP-001, COMP-002]${newline}`, 'components: [COMP-001, COMP-002]\n', ''), '');
  });
}

test('fixture mutations reject a missing template and insert replacement text literally', () => {
  assert.throws(() => replaceFixtureText('owner: other-team\r\n', 'owner: identity-team\n', ''), /Missing fixture text/);
  assert.equal(replaceFixtureText('value\r\n', 'value\n', '$schema: value\n'), '$schema: value\r\n');
});
