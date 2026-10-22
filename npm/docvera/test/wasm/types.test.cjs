'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { writeFileSync } = require('node:fs');
const { resolve, join } = require('node:path');
const { spawnSync } = require('node:child_process');
const { packedPackage } = require('./package-fixture.cjs');

test('installed WASM declarations resolve in NodeNext ESM/CommonJS and browser bundler mode', t => {
  const artifact = packedPackage();
  t.after(artifact.cleanup);
  const esm = `
    import { openChecker, type WorkspaceConfig, type JsonValue } from '@a5i/docvera/wasm';
    import type { PortableChecker } from '@a5i/docvera';
    const config: WorkspaceConfig = { version: 1, include: ['**/*.md'], schemas: {}, frontMatter: { yaml: true } };
    async function example() {
      const checker = await openChecker({ config, sources: { documents: { 'note.md': '# Note' }, assets: [] } });
      const nativeCompatible: PortableChecker = checker;
      const checked = await nativeCompatible.checkWithText('note.md', '# Changed');
      const n: number = checked.documents;
      const snapshot = await checker.sqlSnapshot();
      const value: JsonValue = (await snapshot.querySql('SELECT metadata FROM documents')).rows[0].metadata;
      await snapshot.close();
      await checker.close();
      return [n, value];
    }
    // @ts-expect-error WASM accepts source text, not filesystem roots
    openChecker({ root: '.' });
    // @ts-expect-error unknown parser option
    const bad: WorkspaceConfig = { ...config, parser: { unsupported: true } };
    // @ts-expect-error graph retrieval is native-only, not exposed by WASM
    example().then(() => openChecker({ config, sources: {} })).then(c => c.references({ id: 'A' }));
  `;
  writeFileSync(join(artifact.consumer, 'example.mts'), esm);
  const native = `
    import { openChecker, openWorkspace, type JsonValue, type WorkspaceSources, type QueryResult } from '@a5i/docvera';
    async function nativeExample() {
      const config = { version: 1 as const, include: ['**/*.md'], schemas: {} };
      const sources: WorkspaceSources = { documents: { 'note.md': '# Note' }, shapes: {}, assets: [] };
      const checker = await openChecker({ config, sources });
      const diskChecker = await openChecker({ root: '.' });
      const defaultChecker = await openChecker();
      const value: JsonValue = (await checker.querySql('SELECT metadata FROM documents')).rows[0].metadata;
      const snapshot = await checker.sqlSnapshot();
      const result: QueryResult = await snapshot.querySql('SELECT 1 AS n');
      await checker.references({ id: 'A', direction: 'incoming', depth: 2 });
      await checker.extract('section-id');
      const workspace = await openWorkspace();
      const row: Record<string, JsonValue> = (await workspace.querySql('SELECT metadata FROM documents')).rows[0];
      // @ts-expect-error persistent indexing was removed
      workspace.index();
      // @ts-expect-error full-text search was removed
      workspace.search({ text: 'query' });
      await Promise.all([checker.close(), diskChecker.close(), defaultChecker.close(), snapshot.close(), workspace.close()]);
      return [value, result, row];
    }
    // @ts-expect-error source-based construction requires both config and sources
    openChecker({ config: { version: 1, include: [], schemas: {} } });
  `;
  writeFileSync(join(artifact.consumer, 'native.mts'), native);
  writeFileSync(join(artifact.consumer, 'native.cts'), native);
  writeFileSync(join(artifact.consumer, 'example.cts'), `
    import wasm = require('@a5i/docvera/wasm');
    const checker: Promise<wasm.WasmChecker> = wasm.openChecker({ config: { version: 1, include: [], schemas: {} }, sources: {} });
  `);
  const tsc = resolve(__dirname, '../../../../crates/docvera-wasm/node_modules/typescript/bin/tsc');
  for (const args of [
    ['--module', 'NodeNext', '--moduleResolution', 'NodeNext', 'example.mts', 'example.cts', 'native.mts', 'native.cts'],
    ['--module', 'ESNext', '--moduleResolution', 'Bundler', 'example.mts', 'native.mts'],
  ]) {
    const result = spawnSync(process.execPath, [tsc, '--noEmit', '--strict', '--target', 'ES2022', ...args], {
      cwd: artifact.consumer, encoding: 'utf8', timeout: 30000,
    });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  }
});
