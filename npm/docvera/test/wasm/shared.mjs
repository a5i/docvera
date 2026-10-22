// Public, Promise-based @a5i/docvera/wasm API checks shared by Node and Chromium.
export async function smoke({ openChecker, trace = () => {} }) {
  function assert(value, message) { if (!value) throw new Error(message); }
  function canonical(value) {
    if (Array.isArray(value)) return value.map(canonical);
    if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
    return value;
  }
  function equal(a, b) { assert(JSON.stringify(canonical(a)) === JSON.stringify(canonical(b)), `${JSON.stringify(a)} != ${JSON.stringify(b)}`); }
  async function rejects(fn, pattern, label) {
    let caught = false;
    try { await fn(); } catch (e) {
      assert(e instanceof Error, `Expected JS Error, got ${e}`);
      if (pattern) assert(pattern.test(e.message), e.message);
      caught = true;
    }
    assert(caught, `Expected rejection${label ? ` (${label})` : ''}`);
  }
  const config = { version: 1, include: ['**/*.md'], schemas: { '**/*.md': 'note.yaml' } };
  const schema = `schemaVersion: 1
documentType: note
body: {allowPreamble: true, additionalSections: true}
frontMatter:
  type: object
  required: [id, title]
identity:
  field: frontMatter.id
  uniqueWithin: workspace
`;
  const sources = {
    documents: {
      'a.md': "---\nid: A\ntitle: Привет ' WASM\ntags: [rust, wasm]\n---\n# Alpha\nSee [Beta](b.md).\n",
      'b.md': '---\nid: B\ntitle: Beta\n---\n# Beta\n',
    },
    schemas: { 'note.yaml': schema },
    assets: ['image.png'],
  };
  trace('shacl');
  await rejects(() => openChecker({ config: { ...config, validation: { shacl: { shapes: ['policy.ttl'] } } }, sources }), null, 'shacl');
  // Concurrent construction must share initialization without reusing/freeing the wrong checker.
const [checker, independent] = await Promise.all([
    openChecker({ config, sources }), openChecker({ config, sources: { schemas: sources.schemas } }),
  ]);
  let old;
  trace('opened');
  try {
    equal((await checker.check()).diagnostics, []);
    equal((await independent.check()).documents, 0);
    assert((await checker.checkDocument('a.md', '# Missing\n')).diagnostics.length > 0, 'checkDocument must validate overlay');
    equal((await checker.checkWithText('a.md', '# Missing\n')).documents, 2);
    equal((await checker.check()).diagnostics, []);
  trace('sqlSnapshot');
    old = await checker.sqlSnapshot();
    equal((await old.querySql('SELECT COUNT(*) AS n FROM documents')).rows, [{ n: 2 }]);
    const metadata = (await checker.querySql("SELECT metadata FROM documents WHERE path = 'a.md'")).rows[0].metadata;
    equal(metadata.title, "Привет ' WASM");
    equal(metadata.tags, ['rust', 'wasm']);
    assert(Object.getPrototypeOf(metadata) === Object.prototype, 'MAP must be a plain object');
    equal((await checker.querySql('SELECT d.path, s.heading FROM documents d JOIN sections s ON d.path = s.document_id WHERE s.level = 1 ORDER BY d.path')).rows,
      [{ path: 'a.md', heading: 'Alpha' }, { path: 'b.md', heading: 'Beta' }]);
    equal((await checker.querySql('SELECT parent_id FROM sections WHERE level = 1 LIMIT 1')).rows, [{ parent_id: null }]);
    await rejects(() => checker.querySql('DELETE FROM documents'), null, 'delete');
    await rejects(() => checker.querySql('SELECT 1; SELECT 2'), null, 'multi');
    await rejects(() => checker.updateDocument('../escape.md', '# Escape'), null, 'escape');
    // Errors must not poison the operation queue. Invocation order is deterministic.
  trace('updates');
    const update = checker.updateDocument('a.md', '# Changed\n');
    const schemaUpdate = checker.updateSchema('note.yaml', schema);
    const remove = checker.removeDocument('b.md');
    const query = checker.querySql('SELECT COUNT(*) AS n FROM documents');
    await Promise.all([update, schemaUpdate, remove]);
    equal((await query).rows, [{ n: 1 }]);
    assert((await checker.querySql('SELECT * FROM diagnostics')).rows.length > 0, 'Diagnostics must be queryable');
    equal((await old.querySql('SELECT COUNT(*) AS n FROM documents')).rows, [{ n: 2 }]);
    // close waits for accepted operations, rejects new ones and is idempotent.
  trace('closing');
    // No second checker may keep the shared Worker alive for the snapshot.
    await independent.close();
    const pending = checker.querySql('SELECT declared_id FROM documents');
    const closing = checker.close();
    await rejects(() => checker.check(), /closed/, 'closed-check');
    await rejects(() => checker.sqlSnapshot(), /closed/, 'closed-checker-snapshot');
    assert(checker.close() === closing, 'Repeated checker close must wait for the same cleanup');
    equal((await pending).rows, [{ declared_id: null }]);
    await closing;
    await checker.close();
    equal((await old.querySql('SELECT COUNT(*) AS n FROM documents')).rows, [{ n: 2 }]);
    const oldPending = old.querySql('SELECT COUNT(*) AS n FROM documents');
    const oldClosing = old.close();
    await rejects(() => old.querySql('SELECT 1'), /closed/, 'closed-snapshot');
    equal((await oldPending).rows, [{ n: 2 }]);
    await oldClosing;
    await old.close();
  } finally {
    await old?.close();
    await checker.close();
    await independent.close();
  }
  // Reserve pending snapshots/opens before the final checker begins closing.
  const lone = await openChecker({ config, sources });
  let revision, reopened;
  try {
    const pendingRevision = lone.sqlSnapshot();
    const closing = lone.close();
    const pendingOpen = openChecker({ config, sources });
    [revision, reopened] = await Promise.all([pendingRevision, pendingOpen]);
    await closing;
    equal((await revision.querySql('SELECT COUNT(*) AS n FROM documents')).rows, [{ n: 2 }]);
    equal((await reopened.check()).documents, 2);
  } finally {
    await revision?.close();
    await lone.close();
    await reopened?.close();
  }
  // Opening again after every handle closes must create a usable transport.
  const fresh = await openChecker({ config, sources });
  try { equal((await fresh.check()).documents, 2); }
  finally { await fresh.close(); }
}
