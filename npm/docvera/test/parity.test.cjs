const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, rmSync, readFileSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const { execFileSync } = require('node:child_process');
const { openWorkspace } = require('..');
const repo = resolve(__dirname, '../../..');
const bin = process.env.DOCVERA_BIN || join(repo, 'target/debug', process.platform === 'win32' ? 'docvera.exe' : 'docvera');
function cli(root, ...args) {
  return JSON.parse(execFileSync(bin, ['--root', root, '--format', 'json', ...args], { encoding: 'utf8' }));
}

test('native Node API and CLI return equivalent full-cycle contracts', async () => {
  const root = mkdtempSync(join(tmpdir(), 'docvera-node-'));
  let engine;
  try {
    cli(root, 'init');
    engine = await openWorkspace({ root });
    engine = await openWorkspace({ root });
    assert.deepEqual(await engine.check(), cli(root, 'check'));
    const sql = "SELECT declared_id,type FROM documents ORDER BY path";
    assert.deepEqual(await engine.querySql(sql), cli(root, 'query', 'sql', sql));
    assert.deepEqual(await engine.references({ id: 'FEATURE-0042', direction: 'incoming', depth: 2 }), cli(root, 'graph', 'incoming', 'FEATURE-0042', '--depth', '2'));
    const sections = await engine.querySql("SELECT section_id FROM sections WHERE heading = 'Security'");
    assert.equal(sections.rows.length, 1);
    assert.deepEqual(await engine.extract(sections.rows[0].section_id), cli(root, 'extract', sections.rows[0].section_id));
    await assert.rejects(engine.querySql('DELETE FROM documents'));
    const path = join(root, 'docs/feature-0042.md');
    writeFileSync(path, readFileSync(path, 'utf8').replace('OAuth', 'OpenID'));
    // Retrieval reads current files; no refresh call exists.
    const refreshed = await engine.querySql("SELECT s.section_id, s.text FROM sections s JOIN documents d ON d.path = s.document_id WHERE d.declared_id = 'FEATURE-0042' AND s.heading = 'Security'");
    assert.ok(refreshed.rows[0].text.includes('OpenID'));
    rmSync(path);
    const result = await engine.check();
    assert.ok(result.diagnostics.some(d => d.rule === 'relation.target'));
    assert.equal((await engine.querySql('SELECT count(*) AS n FROM documents')).rows[0].n, 2);
    await engine.close();
    await assert.rejects(engine.check(), /closed/);
  } finally {
    if (engine) await engine.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test('targetTypes alternatives have CLI/native parity and invalidate indexed diagnostics', async () => {
  const root = mkdtempSync(join(tmpdir(), 'docvera-target-types-'));
  let engine;
  try {
    cli(root, 'init');
    const schemaPath = join(root, 'schemas/adr.yaml');
    const original = readFileSync(schemaPath, 'utf8');
    writeFileSync(schemaPath, original.replace('targetType: feature', 'targetTypes: [feature, test-plan]'));
    const documentPath = join(root, 'docs/adr-0001.md');
    writeFileSync(documentPath, readFileSync(documentPath, 'utf8').replace(
      'related: [FEATURE-0042]', 'related: [FEATURE-0042, TEST-0001]'));
    engine = await openWorkspace({ root });
    const valid = await engine.check();
    assert.deepEqual(valid, cli(root, 'check'));
    assert.deepEqual(valid.diagnostics, []);

    writeFileSync(schemaPath, original.replace('targetType: feature', 'targetTypes: [feature]'));
    const invalid = await engine.check();
    assert.ok(invalid.diagnostics.some(d => d.rule === 'relation.target' && d.message.includes('wrong-type')));
    try {
      cli(root, 'check');
      assert.fail('CLI check should reject a wrong target type');
    } catch (error) {
      assert.equal(error.status, 1);
      assert.deepEqual(invalid, JSON.parse(error.stdout));
    }
    const sql = "SELECT rule,message FROM diagnostics WHERE rule='relation.target'";
    const report = await engine.querySql(sql);
    assert.equal(report.rows.length, 1);
    assert.deepEqual(report, cli(root, 'query', 'sql', sql));
  } finally {
    if (engine) await engine.close();
    rmSync(root, { recursive: true, force: true });
  }
});
