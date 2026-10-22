const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const { spawnSync } = require('node:child_process');
const { openWorkspace } = require('..');
const repo = resolve(__dirname, '../../..');
const bin = process.env.DOCVERA_BIN || join(repo, 'target/debug', process.platform === 'win32' ? 'docvera.exe' : 'docvera');
function cli(root, args, code) {
  const result = spawnSync(bin, ['--root', root, '--format', 'json', ...args], { encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, code, result.stderr || result.stdout);
  assert.equal(result.stderr, '');
  return JSON.parse(result.stdout);
}

test('structured SHACL diagnostics, exact UTF-8 spans and SQL persistence have native/CLI parity', async () => {
  const root = mkdtempSync(join(tmpdir(), 'docvera-shacl-spans-'));
  let engine;
  try {
    mkdirSync(join(root, 'docs'));
    writeFileSync(join(root, 'docvera.toml'), "version = 1\ninclude = ['docs/*.md']\n[schemas]\n'docs/*.md' = 'schema.yaml'\n[validation.shacl]\nshapes = ['policy.ttl']\n");
    writeFileSync(join(root, 'schema.yaml'), 'schemaVersion: 1\ndocumentType: note\nbody: {allowPreamble: true, additionalSections: true}\n');
    const source = '---\r\nid: N-1\r\ntitle: Привет 🦀\r\nreview: {states: [draft, planned]}\r\n---\r\n# Note\r\n';
    writeFileSync(join(root, 'docs/note.md'), source);
    writeFileSync(join(root, 'policy.ttl'), '@prefix sh: <http://www.w3.org/ns/shacl#> . <urn:policy:Note> a sh:NodeShape ; sh:targetClass <urn:docvera:v1:type:note> ; sh:property <urn:policy:State> . <urn:policy:State> a sh:PropertyShape ; sh:path <urn:docvera:v1:field:review.states> ; sh:in ("draft") .');
    engine = await openWorkspace({ root });
    const result = await engine.check();
    assert.deepEqual(result, cli(root, ['check'], 1));
    assert.equal(result.diagnostics.length, 1);
    const finding = result.diagnostics[0];
    assert.equal(Buffer.from(source).subarray(finding.span.start, finding.span.end).toString(), 'planned');
    assert.equal(finding.span.line, 4);
    assert.equal(finding.shacl.sourceField, '/review/states/1');
    assert.deepEqual(finding.shacl.resultPath, { kind: 'predicate', iri: 'urn:docvera:v1:field:review.states' });
    assert.deepEqual(finding.shacl.sourceShape, { kind: 'iri', value: 'urn:policy:State' });
    assert.equal(finding.shacl.value.value, 'planned');
    const sql = "SELECT shacl['focusNode']['value'] AS focus, shacl['sourceField'] AS field FROM diagnostics";
    const rows = await engine.querySql(sql);
    assert.deepEqual(rows, cli(root, ['query', 'sql', sql], 0));
    assert.deepEqual(rows.rows, [{ focus: 'urn:docvera:document:id:N-1', field: '/review/states/1' }]);
    assert.deepEqual(await engine.check(), result);
  } finally {
    if (engine) await engine.close();
    rmSync(root, { recursive: true, force: true });
  }
});
