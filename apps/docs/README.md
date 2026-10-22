# docvera documentation site

A standalone **Next.js App Router + MDX** application. It exports static pages and runs real docvera checks and SQL in a browser module Worker. No server functions, native addon execution, external font requests, database or Rust toolchain are needed to build or host the site.

## Develop

Requirements: **Node.js 24**, npm. Python 3 is only needed for the local static preview and browser test server.

```sh
cd apps/docs
npm ci --include=dev
npm run dev
```

Open http://localhost:3000. `predev` copies the browser WASM runtime from the pinned `@a5i/docvera@0.2.0` npm release to ignored `public/runtime/`.

If startup reports a missing `@a5i/docvera`, dependencies are incomplete: rerun `npm ci --include=dev` from this directory. The engine is a required regular dependency; development/build tooling also needs the dev dependencies, even if your shell sets `NODE_ENV=production` or npm defaults to omitting dev packages.

```sh
npm run build       # prepares WASM, then exports to out/
npm start           # previews out/ at http://127.0.0.1:3000 (Python 3)
npm run lint
npm run typecheck   # run after build to include generated route types
npx playwright install chromium
npm run test:e2e    # tests the production export, desktop + mobile Chromium
```

For Linux CI, use `npx playwright install --with-deps chromium`. Browser tests validate all routes, overflow, theme persistence, topic search, actual WASM validation, invalid metadata, SQL results and recovery after query errors. Theme screenshots are written under ignored `test-results/`.

## Deploy to Vercel

1. Import the repository as a new Vercel project.
2. Set **Root Directory** to **`apps/docs`**.
3. Choose **Next.js**, Node.js **24.x**.
4. The included `vercel.json` sets **Install: `npm ci`**, **Build: `npm run build`** and explicit WASM/module MIME types. Leave **Output Directory Override disabled** in Vercel. The Next.js preset uses `.next` for build metadata and automatically handles `output: "export"`; do not override its output directory to `out`.
5. Deploy. No environment variables or credentials are required by this application. Connect a custom domain in the Vercel project settings when ready.

Do not set the repository root as the Vercel application root or run Cargo during the deployment. The build intentionally consumes a published, pinned engine release rather than the current Rust working tree. Deployment to a live Vercel project is a separate operator action.

## Design system

- **shadcn/ui, `base-nova`** with **`@base-ui/react`**, not Radix. Registry components live in `components/ui/`; aliases are declared in `components.json`.
- Tailwind CSS 4 via PostCSS. Brand tokens and responsive layouts live in `app/globals.css`.
- Day: neutral white and cool-gray surfaces with blue navigation/link accents. Night: neutral graphite with soft-blue accents. Green is reserved for successful checks and live status; red signals validation errors.
- `next-themes` follows the OS by default; the day/night toggle persists an explicit preference.
- Geist and Geist Mono are bundled locally through Fontsource, not fetched from Google during build or at runtime.
- Navigation, search dialogs and editor controls support keyboard use. Reduced-motion preferences are respected.

## Content and examples

- `content/*.mdx`: authored documentation. Only trusted repository content is compiled as MDX; user input is never compiled/evaluated as MDX.
- `lib/navigation.ts`: sidebar groups, route metadata and **topic** search (not full-text indexing).
- `app/docs/[[...slug]]/page.tsx`: static route map and table of contents. To add a page, add its MDX import/route map entry and navigation metadata. Use unique plain-text H2 headings for stable TOC anchors.
- `lib/example.ts`: six scenario fixtures (metadata, sections, links, unique IDs, section-tree SQL, workspace reports) with documents, schema, SQL, challenge and error mutation, verified by browser tests.
- `components/playground.tsx`: lazy CodeMirror editor, file tabs and diagnostic/SQL output.
- `public/checker.worker.js`: imports only the public portable browser API. Each run gets an isolated Worker with a 20-second timeout. Workers are terminated on completion, failure and component unmount, so synchronous WASM cannot block the UI. Startup/compilation is included in the displayed elapsed time.
- `scripts/prepare-wasm.mjs`: copies browser runtime assets only. Generated WASM assets are never committed. Update the pinned dependency and script version guard together, then rerun browser tests.

The playground offers six curated workspaces with one to three editable Markdown documents and one shared schema per scenario, not arbitrary filesystem loading. Changing scenarios asks before discarding edits, even if they have already been checked. Reset restores the selected scenario. Compact embedded examples keep the introductory note. Changes are temporary; there is no persistence or share-link backend. SQL is read-only. SHACL/RDF and filesystem operations are not supported by the WASM backend.

## Dependency audit

`npm audit --omit=dev` checks the shipped dependency tree. At initial setup the full audit also reports an upstream `braces`/`fast-glob` advisory through `eslint-config-next`; it is build/lint tooling, not browser runtime code. Do not apply `npm audit fix --force` blindly: its proposed fix downgrades the Next.js ESLint configuration across major versions. Review upstream fixes separately.
