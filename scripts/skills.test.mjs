import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const cli = resolve(process.env.DOCVERA_BIN || join(root, 'target/debug', process.platform === 'win32' ? 'docvera.exe' : 'docvera'));
const names = ['docvera-node-wasm', 'docvera-overview', 'docvera-policies', 'docvera-sql-query', 'docvera-workspace'];
const skill = name => readFileSync(join(root, 'skills', name, 'SKILL.md'), 'utf8');
const blocks = (text, language) => [...text.matchAll(new RegExp('^```' + language + '\\r?\\n([\\s\\S]*?)^```$', 'gm'))].map(m => m[1]);

function scratch(t) {
  const dir = mkdtempSync(join(tmpdir(), 'docvera-skills-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function run(args, cwd, status = 0) {
  const result = spawnSync(cli, args, { cwd, encoding: 'utf8', timeout: 30_000, maxBuffer: 4 * 1024 * 1024 });
  assert.ifError(result.error);
  assert.equal(result.status, status, `${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
  assert.equal(result.stderr, '');
  return JSON.parse(result.stdout);
}

// Intentionally not a shell: only standalone commands and simple quoted arguments.
// Reject expansion/control operators so new examples cannot execute arbitrary code.
function argumentsOf(line) {
  assert.doesNotMatch(line, /[$`\\;|&<>]/);
  const words = line.match(/"[^"]*"|'[^']*'|[^\s"']+/g) || [];
  assert.equal(words.join(' '), line, `Unsupported example syntax: ${line}`);
  assert.equal(words.shift(), 'docvera');
  return words.map(word => /^["']/.test(word) ? word.slice(1, -1) : word);
}

function validateBundle(name, text) {
  assert.match(name, /^docvera-[a-z0-9]+(?:-[a-z0-9]+)*$/);
  assert.doesNotMatch(text, /\r|[\u200B-\u200F\u202A-\u202E\u2066-\u2069\uFEFF]/);
  const header = text.match(/^---\nname: ([a-z0-9-]+)\ndescription: ([^\n]+)\n---\n/);
  assert.ok(header, `${name}: use simple portable name/description frontmatter`);
  assert.equal(header[1], name);
  assert.ok(header[2].length > 20 && header[2].length <= 1024);
  assert.match(header[0], /^[\x00-\x7F]+$/); // APM portable frontmatter subset.
  assert.ok(text.split('\n').length <= 180, `${name}: keep procedures thin`);
  assert.match(text, /## /);
  assert.ok(text.endsWith('\n'));
  // Bundles must be independently installable: no resources outside their root.
  for (const [, href] of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    if (/^https:\/\//.test(href) || href.startsWith('#')) continue;
    assert.ok(!isAbsolute(href));
    const bundle = realpathSync(join(root, 'skills', name));
    const target = realpathSync(resolve(bundle, href.split('#')[0]));
    const rel = relative(bundle, target);
    assert.ok(rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel), href);
  }
}

test('five independent portable bundles have valid frontmatter and resources', () => {
  assert.deepEqual(readdirSync(join(root, 'skills'), { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name).sort(), names);
  for (const name of names) validateBundle(name, skill(name));
});

test('bundle checks reject broken names, missing descriptions, CRLF and oversized descriptions', () => {
  const text = skill('docvera-overview');
  for (const broken of [text.replace('name: docvera-overview', 'name: wrong'), text.replace(/^description:.*\n/m, ''), text.replaceAll('\n', '\r\n'), text.replace(/^description:.*$/m, `description: ${'x'.repeat(1025)}`)]) {
    assert.throws(() => validateBundle('docvera-overview', broken));
  }
  assert.throws(() => argumentsOf('docvera check; touch bad'));
});

test('every fenced CLI example runs and covers the installed command contract', t => {
  const cwd = scratch(t);
  const guide = run(['guide', '--format', 'json'], cwd);
  assert.equal(guide.contractVersion, 1);
  const covered = new Set();
  let commands = 0;
  for (const name of names) {
    // Each skill gets a fresh workspace so examples cannot depend on siblings.
    const consumer = join(cwd, name);
    mkdirSync(consumer);
    if (name !== 'docvera-workspace') run(['--root', 'demo', 'init', '--format', 'json'], consumer);
    for (const block of blocks(skill(name), 'sh')) {
      for (const line of block.trim().split('\n')) {
        const args = argumentsOf(line);
        const commandArgs = args.filter((arg, i) => !['--root', '--format'].includes(args[i - 1]) && !arg.startsWith('--'));
        const command = commandArgs[0] === 'query' ? commandArgs.slice(0, 2).join(' ') : commandArgs[0];
        if (!command) {
          assert.deepEqual(args, ['--version']);
          const version = spawnSync(cli, args, { cwd: consumer, encoding: 'utf8', timeout: 30_000 });
          assert.ifError(version.error);
          assert.equal(version.status, 0);
          assert.match(version.stdout, /^docvera /);
          continue;
        }
        assert.ok(Object.hasOwn(guide.contract.commands, command), command);
        covered.add(command);
        const result = run(args, consumer);
        if (command === 'schema') {
          assert.equal(result.$schema, 'https://json-schema.org/draft/2020-12/schema');
          assert.equal(result.contractVersion, undefined);
        } else {
          assert.equal(result.contractVersion, guide.contractVersion);
          const key = command === 'query sql' ? 'query' : command;
          for (const field of guide.contract.results[key] || []) assert.ok(Object.hasOwn(result, field), `${command}.${field}`);
          if (command === 'check') assert.equal(result.documents, 3);
          if (command === 'extract') assert.match(result.markdown, /Security/);
          if (command === 'query sql' && !line.includes('diagnostics')) assert.ok(result.rows.length > 0);
        }
        commands++;
      }
    }
  }
  assert.deepEqual([...covered].sort(), Object.keys(guide.contract.commands).sort());
  t.diagnostic(`Executed ${commands} documented CLI commands against the installed contract`);
});

test('schema and SHACL examples pass, fail, repair and reject malformed policies', t => {
  const cwd = scratch(t);
  for (const dir of ['docs', 'schemas', 'policies']) mkdirSync(join(cwd, dir));
  const text = skill('docvera-policies');
  assert.equal(blocks(text, 'yaml').length, 1);
  assert.equal(blocks(text, 'turtle').length, 1);
  writeFileSync(join(cwd, 'schemas/note.yaml'), blocks(text, 'yaml')[0]);
  const policy = blocks(text, 'turtle')[0];
  const policyPath = join(cwd, 'policies/note.ttl');
  writeFileSync(policyPath, policy);
  writeFileSync(join(cwd, 'docvera.toml'), 'version = 1\ninclude = ["docs/**/*.md"]\n[schemas]\n"docs/**/*.md" = "schemas/note.yaml"\n[validation.shacl]\nshapes = ["policies/note.ttl"]\n');
  const document = join(cwd, 'docs/note.md');
  const valid = '---\nid: NOTE-1\nowner: team-a\n---\n# Note\n';
  writeFileSync(document, valid);
  assert.deepEqual(run(['check', '--format', 'json'], cwd).diagnostics, []);
  writeFileSync(document, valid.replace('owner: team-a\n', ''));
  const findings = run(['check', '--format', 'json'], cwd, 1).diagnostics;
  assert.ok(findings.some(d => d.rule.startsWith('schema.')));
  assert.ok(findings.some(d => d.rule === 'shacl.MinCountConstraintComponent' && d.shacl));
  writeFileSync(document, valid);
  assert.deepEqual(run(['check', '--warnings-as-errors', '--format', 'json'], cwd).diagnostics, []);
  writeFileSync(policyPath, 'not valid Turtle {');
  assert.equal(run(['check', '--format', 'json'], cwd, 2).error.code, 'operation_failed');
  rmSync(policyPath);
  assert.equal(run(['check', '--format', 'json'], cwd, 2).error.code, 'operation_failed');
});

test('SQL failure is an operation error; live source edits are immediately visible', t => {
  const cwd = scratch(t);
  run(['init', '--format', 'json'], cwd);
  assert.equal(run(['query', 'sql', 'SELECT nonexistent FROM documents', '--format', 'json'], cwd, 2).error.code, 'operation_failed');
  assert.equal(run(['query', 'sql', 'DELETE FROM documents', '--format', 'json'], cwd, 2).error.code, 'operation_failed');
  const path = join(cwd, 'docs/feature-0042.md');
  writeFileSync(path, readFileSync(path, 'utf8').replace('## Security', '## Missing security'));
  assert.ok(run(['check', '--format', 'json'], cwd, 1).diagnostics.some(d => d.rule.startsWith('body.')));
});

test('exact JavaScript examples run through native, WASM and Node Worker APIs', t => {
  const cwd = scratch(t);
  run(['--root', 'demo', 'init', '--format', 'json'], cwd);
  const scope = join(cwd, 'node_modules/@a5i');
  mkdirSync(scope, { recursive: true });
  symlinkSync(join(root, 'npm/docvera'), join(scope, 'docvera'), process.platform === 'win32' ? 'junction' : 'dir');
  const examples = blocks(skill('docvera-node-wasm'), 'js');
  assert.equal(examples.length, 3);
  assert.ok(existsSync(join(root, 'npm/docvera/wasm/node/docvera_wasm_bg.wasm')), 'Build the WASM runtime before running skill examples');
  for (const [name, code] of [
    ['native', examples[0]],
    ['wasm', examples[1]],
    ['worker', examples[1].replace('import { openChecker }', 'import { openCheckerWorker as openChecker }')],
    ['positions', examples[2]],
  ]) {
    const file = join(cwd, `${name}.mjs`);
    writeFileSync(file, code);
    const result = spawnSync(process.execPath, [file], { cwd, encoding: 'utf8', timeout: 60_000 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, `${name}: ${result.stdout}\n${result.stderr}`);
    assert.equal(result.stderr, '');
    if (name === 'native') assert.match(result.stdout, /docs\/feature-0042.md/);
    else if (name === 'positions') {
      const rows = result.stdout.trim().split('\n').map(line => JSON.parse(line));
      assert.ok(rows.length > 0);
      for (const row of rows) {
        const source = readFileSync(join(cwd, 'demo', row.document_id));
        assert.equal(row.markdown, source.subarray(row.span.start, row.span.end).toString('utf8'));
        assert.ok(['paragraph', 'code', 'list', 'table', 'blockquote'].includes(row.kind));
      }
    }
    else assert.match(result.stdout, /NOTE-1[\s\S]*NOTE-2/);
  }
});
