import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const skillsVersion = '1.7.4';
const apmVersion = '0.33.0';
const names = readdirSync(join(root, 'skills'), { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name).sort();

function sandbox(t) {
  const dir = mkdtempSync(join(tmpdir(), 'docvera-installers-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const source = join(dir, 'source');
  mkdirSync(source);
  // Include the real root manifest to catch package-intent/discovery collisions.
  cpSync(join(root, 'apm.yml'), join(source, 'apm.yml'));
  cpSync(join(root, 'skills'), join(source, 'skills'), { recursive: true });
  const home = join(dir, 'home');
  mkdirSync(home);
  const env = {
    ...process.env,
    HOME: home, USERPROFILE: home,
    XDG_CONFIG_HOME: join(home, 'config'), XDG_CACHE_HOME: join(home, 'cache'),
    XDG_DATA_HOME: join(home, 'data'), XDG_STATE_HOME: join(home, 'state'),
    APM_HOME: join(home, '.apm'),
    npm_config_cache: join(dir, 'npm-cache'),
    DISABLE_TELEMETRY: '1', DO_NOT_TRACK: '1', CI: '1'
  };
  const run = (bin, args, cwd) => {
    const result = spawnSync(bin, args, { cwd, env, encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, `${bin} ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
    return result.stdout;
  };
  return { dir, source, run };
}

function assertDeployment(consumer, expected) {
  const deployed = join(consumer, '.agents/skills');
  assert.ok(existsSync(deployed), `No deployment in ${deployed}`);
  assert.deepEqual(readdirSync(deployed).sort(), expected);
  for (const name of expected) {
    const files = readdirSync(join(root, 'skills', name), { recursive: true }).sort();
    assert.deepEqual(readdirSync(join(deployed, name), { recursive: true }).sort(), files);
    for (const file of files) {
      // Current bundles contain only files; extend deliberately when adding resources.
      assert.deepEqual(readFileSync(join(deployed, name, file)), readFileSync(join(root, 'skills', name, file)));
    }
  }
  assert.ok(!existsSync(join(deployed, 'apm.yml')));
  assert.ok(!existsSync(join(deployed, 'README.md')));
}

test(`skills@${skillsVersion} discovers and installs full/selective collections`, t => {
  const { dir, source, run } = sandbox(t);
  const npx = process.env.NPX_BIN || 'npx';
  assert.match(run(npx, ['--yes', `skills@${skillsVersion}`, '--version'], dir), new RegExp(skillsVersion.replaceAll('.', '\\.')));
  for (const subset of [false, true]) {
    const consumer = join(dir, subset ? 'selective' : 'full');
    mkdirSync(consumer);
    const args = ['--yes', `skills@${skillsVersion}`, 'add', source, '--agent', 'codex', '--yes'];
    args.push('--skill', subset ? 'docvera-sql-query' : '*');
    run(npx, args, consumer);
    assertDeployment(consumer, subset ? ['docvera-sql-query'] : names);
  }
});

test(`APM ${apmVersion} installs full/selective collections and reinstalls locked dependencies`, t => {
  const { dir, source, run } = sandbox(t);
  const apm = process.env.APM_BIN || 'apm';
  assert.match(run(apm, ['--version'], dir), new RegExp(`version ${apmVersion.replaceAll('.', '\\.')}(?:\\s|$)`));
  for (const subset of [false, true]) {
    const consumer = join(dir, subset ? 'selective' : 'full');
    mkdirSync(consumer);
    const args = ['install', source, '--target', 'agent-skills'];
    if (subset) args.push('--skill', 'docvera-sql-query');
    run(apm, args, consumer);
    const expected = subset ? ['docvera-sql-query'] : names;
    assertDeployment(consumer, expected);
    const lockPath = join(consumer, 'apm.lock.yaml');
    const manifestPath = join(consumer, 'apm.yml');
    assert.ok(existsSync(lockPath), 'APM must write a reproducible lockfile');
    const lock = readFileSync(lockPath, 'utf8');
    const manifest = readFileSync(manifestPath, 'utf8');
    if (subset) {
      assert.match(manifest, /skills:/);
      assert.match(manifest, /docvera-sql-query/);
      assert.match(lock, /skill_subset:/);
    }
    rmSync(join(consumer, '.agents'), { recursive: true });
    run(apm, ['install', '--frozen', '--target', 'agent-skills'], consumer);
    assertDeployment(consumer, expected);
    assert.equal(readFileSync(lockPath, 'utf8'), lock);
    assert.equal(readFileSync(manifestPath, 'utf8'), manifest);
  }
});
