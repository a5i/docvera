// Identical checks execute in Node and in a real browser against the WASM artifact.
export function smoke({ Checker }) {
  function assert(value, message) { if (!value) throw new Error(message); }
  function canonical(value) {
    if (Array.isArray(value)) return value.map(canonical);
    if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
    return value;
  }
  function equal(actual, expected) {
    assert(JSON.stringify(canonical(actual)) === JSON.stringify(canonical(expected)), `${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
  }
  function throws(fn) {
    let thrown = false;
    try { fn(); } catch (e) { assert(e instanceof Error, `Expected Error, got ${e}`); thrown = true; }
    assert(thrown, 'Expected operation to throw');
  }
  const config = { version: 1, include: ['**/*.md'], schemas: { '**/*.md': 'schemas/note.yaml' } };
  const schema = `schemaVersion: 1
documentType: note
body: {allowPreamble: true, additionalSections: true}
frontMatter:
  type: object
  required: [id, title]
  properties:
    id: {type: string}
    title: {type: string}
identity:
  field: frontMatter.id
  uniqueWithin: workspace
`;
  const a = "---\nid: A\ntitle: Привет ' WASM\ntags: [rust, wasm]\n---\n# Alpha\nSee [Beta](b.md).\n";
  const checker = new Checker(config, {
    documents: { 'a.md': a, 'b.md': '---\nid: B\ntitle: Beta\n---\n# Beta\n' },
    schemas: { 'schemas/note.yaml': schema }
  });
  let old, fresh;
  try {
    equal(checker.check().diagnostics, []);
    const overlay = checker.checkDocument('a.md', '# Missing metadata\n');
    assert(overlay.diagnostics.length > 0, 'Overlay validation must work');
    equal(checker.check().diagnostics, []);
    old = checker.sqlSnapshot();
    equal(old.query('SELECT COUNT(*) AS n FROM documents').rows, [{ n: 2 }]);
    const rows = old.query("SELECT metadata, declared_id FROM documents WHERE path = 'a.md'").rows;
    equal(rows[0].metadata.tags, ['rust', 'wasm']);
    equal(rows[0].metadata.title, "Привет ' WASM");
    equal(rows[0].declared_id, 'A');
    assert(Object.getPrototypeOf(rows[0].metadata) === Object.prototype, 'MAP must serialize as plain object');
    equal(old.query("SELECT metadata['title'] AS title FROM documents WHERE path = 'b.md'").rows, [{ title: 'Beta' }]);
    equal(old.query('SELECT d.path, s.heading FROM documents d JOIN sections s ON d.path = s.document_id WHERE s.level = 1 ORDER BY d.path').rows,
      [{ path: 'a.md', heading: 'Alpha' }, { path: 'b.md', heading: 'Beta' }]);
    equal(old.query("SELECT parent_id FROM sections WHERE level = 1 LIMIT 1").rows, [{ parent_id: null }]);
    for (const query of ['DELETE FROM documents', 'SELECT 1; SELECT 2', 'SELECT * FROM missing',
      'SELECT path AS x, source AS x FROM documents']) throws(() => old.query(query));
    throws(() => checker.updateDocument('../escape.md', '# Escape'));
    checker.removeDocument('b.md');
    checker.updateDocument('a.md', '# Changed\n');
    checker.updateSchema('schemas/note.yaml', schema);
    fresh = checker.sqlSnapshot();
    equal(old.query('SELECT COUNT(*) AS n FROM documents').rows, [{ n: 2 }]);
    equal(fresh.query('SELECT COUNT(*) AS n FROM documents').rows, [{ n: 1 }]);
    assert(fresh.query('SELECT * FROM diagnostics').rows.length > 0, 'Diagnostics must be queryable');
    equal(fresh.query('SELECT declared_id FROM documents').rows, [{ declared_id: null }]);
    throws(() => new Checker({ ...config, validation: { shacl: { shapes: ['rules.ttl'] } } }, {}));
  } finally {
    fresh?.free();
    old?.free();
    checker.free();
  }
}
