const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { openChecker, openWorkspace } = require('..');

for (const newline of ['\n', '\r\n']) {
  for (const backend of ['workspace', 'checker']) {
    test(`${backend} exposes exact block and section positions with ${JSON.stringify(newline)}`, async t => {
      const source = [
        '---', 'id: NOTE-1', '---', '# Привет 🦀', '',
        'Текст **жирный** и `код`.', '',
        '```js title', 'const x = "🦀";', '```', '',
        '- пункт 🦀', '- второй', '',
        '| Имя |', '| --- |', '| 🦀 |', '',
        '> цитата 🦀', '', '## Child', '', 'Последний 🦀 текст',
      ].join(newline); // Deliberately no final newline.
      const schema = JSON.stringify({
        schemaVersion: 1, documentType: 'note',
        frontMatter: { type: 'object', required: ['id'] },
        body: { allowPreamble: true, additionalSections: true },
      });
      let api;
      if (backend === 'workspace') {
        const root = mkdtempSync(join(tmpdir(), 'docvera-block-positions-'));
        t.after(() => rmSync(root, { recursive: true, force: true }));
        writeFileSync(join(root, 'docvera.toml'), 'version = 1\ninclude = ["*.md"]\n[schemas]\n"*.md" = "note.yaml"\n');
        writeFileSync(join(root, 'note.yaml'), schema);
        writeFileSync(join(root, 'note.md'), source);
        api = await openWorkspace({ root });
      } else {
        api = await openChecker({
          config: { version: 1, include: ['*.md'], schemas: { '*.md': 'note.yaml' } },
          sources: { documents: { 'note.md': source }, schemas: { 'note.yaml': schema } },
        });
      }
      t.after(() => api.close());
      assert.deepEqual((await api.check()).diagnostics, []);
      const bytes = Buffer.from(source, 'utf8');
      function expectedSpan(fragment, line, column, endLine, endColumn) {
        const index = source.indexOf(fragment);
        assert.ok(index >= 0, fragment);
        assert.equal(source.indexOf(fragment, index + 1), -1, 'fixture fragment must be unique');
        const start = Buffer.byteLength(source.slice(0, index), 'utf8');
        return { start, end: start + Buffer.byteLength(fragment, 'utf8'), line, column, endLine, endColumn };
      }
      const expected = [
        ['paragraph', null, `Текст **жирный** и \`код\`.${newline}`, 6, 1, 7, 1],
        ['code', 'js', ['```js title', 'const x = "🦀";', '```'].join(newline), 8, 1, 10, 4],
        ['list', null, ['- пункт 🦀', '- второй', '', ''].join(newline), 12, 1, 15, 1],
        ['table', null, ['| Имя |', '| --- |', '| 🦀 |', ''].join(newline), 15, 1, 18, 1],
        ['blockquote', null, `> цитата 🦀${newline}`, 19, 1, 20, 1],
        ['paragraph', null, `цитата 🦀${newline}`, 19, 3, 20, 1],
        ['paragraph', null, 'Последний 🦀 текст', 23, 1, 23, 18],
      ];
      const result = await api.querySql("SELECT document_id, section_id, kind, language, span FROM nodes ORDER BY span['start']");
      assert.equal(result.contractVersion, 1);
      // Exact kinds/count also guard against treating this projection as a full inline AST.
      assert.equal(result.rows.length, expected.length);
      for (const [i, [kind, language, fragment, ...coordinates]] of expected.entries()) {
        const row = result.rows[i];
        assert.equal(row.document_id, 'note.md');
        assert.equal(row.kind, kind);
        assert.equal(row.language, language);
        assert.deepEqual(row.span, expectedSpan(fragment, ...coordinates), kind);
        assert.equal(bytes.subarray(row.span.start, row.span.end).toString('utf8'), fragment);
      }
      const sections = (await api.querySql('SELECT section_id, heading, text, span FROM sections ORDER BY ordinal')).rows;
      assert.equal(sections.length, 2);
      const parentStart = source.indexOf('# Привет');
      const childStart = source.indexOf('## Child');
      assert.deepEqual(sections[0].span, expectedSpan(source.slice(parentStart), 4, 1, 23, 18));
      assert.deepEqual(sections[1].span, expectedSpan(source.slice(childStart), 21, 1, 23, 18));
      assert.equal(sections[0].heading, 'Привет 🦀');
      assert.equal(sections[1].heading, 'Child');
      assert.ok(!sections[0].text.includes('Последний'), 'direct text excludes child sections');
      for (const row of result.rows.slice(0, -1)) assert.equal(row.section_id, sections[0].section_id);
      assert.equal(result.rows.at(-1).section_id, sections[1].section_id);
      for (const section of sections) {
        const extracted = await api.extract(section.section_id);
        assert.deepEqual(extracted.location, section.span);
        assert.equal(extracted.markdown, bytes.subarray(section.span.start, section.span.end).toString('utf8'));
      }
    });
  }
}
