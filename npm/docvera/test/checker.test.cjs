const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, rmSync, writeFileSync, readFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const { spawnSync } = require('node:child_process');
const { openChecker } = require('..');
const repo = resolve(__dirname, '../../..');
const bin = process.env.DOCVERA_BIN || join(repo, 'target/debug', process.platform === 'win32' ? 'docvera.exe' : 'docvera');
function check(root) {
  const result = spawnSync(bin, ['--root', root, '--format', 'json', 'check'], { encoding: 'utf8' });
  assert.ok(result.status === 0 || result.status === 1, result.stderr);
  return JSON.parse(result.stdout);
}
function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'docvera-checker-'));
  writeFileSync(join(root, 'docvera.toml'), `version = 1
include = ['*.md']
[schemas]
'n*.md' = 'note.yaml'
[validation]
untyped = 'skip'
[parser]
tables = false
wikilinks = true
`);
  writeFileSync(join(root, 'note.yaml'), `schemaVersion: 1
documentType: note
identity: {field: frontMatter.id, uniqueWithin: workspace}
body: {allowPreamble: true, additionalSections: true}
relations:
  - {from: frontMatter.related, targetType: note}
`);
  writeFileSync(join(root, 'n1.md'), '---\nid: N-1\n---\n# Привет 🦀\n');
  writeFileSync(join(root, 'n2.md'), '---\nid: N-2\nrelated: N-1\n---\n# Other\n');
  return root;
}

test('cached checker overlays match CLI, preserve bytes and survive removal of workspace', async () => {
  const root = fixture();
  let checker;
  try {
    checker = await openChecker({ root });
    const before = readFileSync(join(root, 'n1.md'), 'utf8');
    assert.deepEqual(await checker.check(), check(root));
    const edited = '\ufeff+++\r\nid = "N-3"\r\nrelated = ["N-2", "MISSING"]\r\n+++\r\n# Привет 🦀\r\n';
    const one = await checker.checkDocument('n1.md', edited);
    assert.equal(readFileSync(join(root, 'n1.md'), 'utf8'), before);
    assert.ok(one.diagnostics.some(d => d.rule === 'relation.target'));
    const span = one.diagnostics.find(d => d.rule === 'relation.target').span;
    assert.equal(Buffer.from(edited).subarray(span.start, span.end).toString(), '"MISSING"');
    writeFileSync(join(root, 'n1.md'), edited);
    const expected = check(root);
    assert.deepEqual(one.diagnostics, expected.diagnostics.filter(d => d.path === 'n1.md'));
    assert.deepEqual(await checker.checkWithText('n1.md', edited), expected);
    assert.deepEqual((await checker.check()).diagnostics, [], 'overlay must not change baseline');
    rmSync(root, { recursive: true, force: true });
    assert.deepEqual(await checker.checkWithText('n1.md', edited), expected);
    await checker.updateDocument('n1.md', edited);
    assert.deepEqual(await checker.check(), expected);
    await checker.removeDocument('n1.md');
    assert.ok((await checker.check()).diagnostics.some(d => d.path === 'n2.md'));
    await assert.rejects(checker.checkDocument('../escape.md', edited));
    const pending = checker.checkDocument('n2.md', '---\nid: N-2\n---\n# Other\n');
    await checker.close();
    assert.deepEqual((await pending).diagnostics, []);
    await assert.rejects(checker.check(), /closed/);
  } finally {
    if (checker) await checker.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test('untyped skip, new buffers and configured parser have native/CLI parity', async () => {
  const root = fixture();
  let checker;
  try {
    checker = await openChecker({ root });
    const skipped = '[bad](missing.md)\n';
    assert.deepEqual((await checker.checkDocument('free.md', skipped)).diagnostics, []);
    await checker.updateDocument('free.md', skipped);
    const linked = '---\nid: N-1\n---\n# Note\n\n[[missing.md]]\n';
    writeFileSync(join(root, 'free.md'), skipped);
    writeFileSync(join(root, 'n1.md'), linked);
    assert.deepEqual(await checker.checkWithText('n1.md', linked), check(root));
    const selected = '---\n$schema: new.yaml\nid: N-3\n---\n# New\n';
    assert.ok((await checker.checkDocument('n3.md', selected)).diagnostics.some(d => d.rule === 'schema.resolve'));
    await checker.updateSchema('new.yaml', readFileSync(join(root, 'note.yaml'), 'utf8'));
    assert.deepEqual((await checker.checkDocument('n3.md', selected)).diagnostics, []);
    await checker.updateSchema('note.yaml', 'schemaVersion: 999\ndocumentType: bad\n');
    assert.ok((await checker.check()).diagnostics.some(d => d.rule === 'schema.resolve'));
    await checker.updateSchema('note.yaml', readFileSync(join(root, 'note.yaml'), 'utf8'));
    const fresh = await checker.checkDocument('n3.md', '---\nid: N-3\n---\n# New\n');
    assert.equal(fresh.documents, 1);
    assert.deepEqual(fresh.diagnostics, []);
  } finally {
    if (checker) await checker.close();
    rmSync(root, { recursive: true, force: true });
  }
});
