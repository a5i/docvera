# docvera-wasm (internal build crate)

Thin `wasm-bindgen` adapter for the Docvera checker and read-only GlueSQL snapshots. No SQLite, FTS, RDF, filesystem loading or native addon. This crate is **not a separately published npm package**: its Node/web artifacts ship inside **`@a5i/docvera`**, exposed as `@a5i/docvera/wasm`.

See [in-memory SQL and WASM](../../docs/in-memory-sql.md) for the public API, table schema, build instructions and safety boundaries.

Build and test the public package from the repository root:

```sh
npm --prefix crates/docvera-wasm ci
npm --prefix npm/docvera run build:wasm
npm --prefix npm/docvera run test:wasm
npm --prefix crates/docvera-wasm exec -- playwright install chromium # --with-deps on CI
npm --prefix npm/docvera run test:wasm:browser # CHROMIUM overrides browser executable
```

The private package.json here supplies pinned build/test tools only. `npm run build` produces intermediate `pkg-node/` and `pkg-web/` files; the main package assembly script copies runtime files into `npm/docvera/wasm/{node,web}`. Generated raw classes use `.free()`, but public consumers use `await checker.close()` / `await snapshot.close()` instead.

Promise-based JS operations still execute synchronous WASM on the calling thread; use a Web Worker for browser UI integration. SQL snapshots are copied, read-only and explicit: recreate them after baseline edits.
