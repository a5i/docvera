const { test } = require('node:test');
const assert = require('node:assert/strict');
const { cpSync, mkdtempSync, readdirSync, readFileSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve, basename } = require('node:path');
const { spawnSync } = require('node:child_process');
const { openWorkspace } = require('..');
const repo = resolve(__dirname, '../../..');
const bin = process.env.MDX_BIN || join(repo, 'target/debug', process.platform === 'win32' ? 'mdx.exe' : 'mdx');
function cli(root, args, code = 0) {
  const output = spawnSync(bin, ['--root', root, '--format', 'json', ...args], { encoding: 'utf8' });
  assert.ifError(output.error);
  assert.equal(output.status, code, output.stderr || output.stdout);
  assert.equal(output.stderr, '');
  return JSON.parse(output.stdout);
}

for (const [name, count, file, from, to] of [
  ['product-traceability', 6, 'feature-draft.md', 'status: draft', 'status: planned'],
  ['qa-traceability', 6, 'requirement-uncovered.md', 'priority: normal', 'priority: critical'],
  ['operational-readiness', 4, 'service.md', 'runbook: RUN-001\n', ''],
  ['architecture-governance', 5, 'adr.md', 'components: [COMP-001, COMP-002]\n', ''],
  ['project-governance', 4, 'specification.md', 'status: approved', 'status: draft'],
]) {
  test(`${name}: shared fixtures, policy reports and native/CLI parity`, async () => {
    const root = mkdtempSync(join(tmpdir(), 'mdx-case-'));
    let engine;
    try {
      cpSync(join(repo, 'examples/use-cases', name), root, {
        recursive: true, filter: source => basename(source) !== '.mdx',
      });
      engine = await openWorkspace({ root });
      const check = await engine.check();
      assert.deepEqual(check, cli(root, ['check']));
      assert.deepEqual(check.diagnostics, []);
      const indexed = await engine.index();
      assert.equal(indexed.updated, count);
      assert.equal(cli(root, ['index']).updated, 0);
      const compareReports = async () => {
        for (const file of readdirSync(join(root, 'queries')).filter(file => file.endsWith('.sql')).sort()) {
          const sql = readFileSync(join(root, 'queries', file), 'utf8');
          assert.deepEqual(await engine.querySql(sql), cli(root, ['query', 'sql', sql]));
        }
      };
      await compareReports();
      const path = join(root, 'docs', file);
      const original = readFileSync(path, 'utf8');
      assert.ok(original.includes(from));
      writeFileSync(path, original.replace(from, to));
      assert.deepEqual((await engine.check()).diagnostics, []);
      await engine.index();
      await compareReports();
      if (name === 'operational-readiness') {
        const options = { text: 'database connection timeout', documentType: 'runbook', section: 'Diagnosis' };
        const hits = await engine.search(options);
        assert.deepEqual(hits, cli(root, ['search', options.text, '--type', 'runbook', '--section', 'Diagnosis']));
        assert.equal(hits.results.length, 1);
        const section = hits.results[0].sectionId;
        assert.deepEqual(await engine.extract(section), cli(root, ['extract', section]));
      }
      if (name === 'architecture-governance') {
        for (const [id, direction] of [['COMP-001', 'incoming'], ['COMP-002', 'outgoing']]) {
          assert.deepEqual(await engine.references({ id, direction, depth: 1 }), cli(root, ['graph', direction, id, '--depth', '1']));
        }
      }
      writeFileSync(path, original.replace(/^owner: .+\n/m, ''));
      const invalid = await engine.check();
      assert.ok(invalid.diagnostics.length > 0);
      assert.deepEqual(invalid, cli(root, ['check'], 1));
      await engine.index();
      assert.equal(cli(root, ['index']).updated, 0);
    } finally {
      if (engine) await engine.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
}
