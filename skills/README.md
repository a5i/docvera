# Docvera skills for AI agents

Portable, end-user workflows for working **with docvera**, not contributor or
maintainer instructions. The canonical sources are `skills/<name>/SKILL.md` in
[a5i/docvera](https://github.com/a5i/docvera). Both installers below consume these
same files; no npm skills package, plugin, or MCP server is required.

| Skill | Purpose |
| --- | --- |
| `docvera-overview` | Verify capabilities, choose commands, interpret results and trust boundaries. |
| `docvera-workspace` | Initialize, author, diagnose, and validate a workspace or CI gate. |
| `docvera-sql-query` | Discover documents, query GlueSQL, trace references and extract evidence. |
| `docvera-policies` | Author/test document schemas and SHACL invariants without weakening requirements. |
| `docvera-node-wasm` | Choose native/WASM/Worker APIs and manage updates, snapshots and cleanup. |

Each skill works independently. The overview helps discovery but is not a
mandatory dependency. Syntax truth lives in the installed `docvera guide` and
`contract`; JavaScript API truth lives in the installed package's declarations.
Skills describe workflows, not an alternate copy of those references.

## Install with the skills CLI

Requires a Node.js version supported by the skills CLI:

```sh
npx skills add a5i/docvera
npx skills add a5i/docvera --skill docvera-sql-query
```

The first command offers skill/harness selection. Installation is project-local
by default; add `-g` for user scope. For a non-interactive harness-specific install,
for example Codex:

```sh
npx skills add a5i/docvera --skill docvera-workspace --agent codex --yes
```

Use `npx skills check` and `npx skills update` to manage updates. Retain the
installer's generated lockfiles where applicable. See the upstream
[CLI reference](https://github.com/vercel-labs/skills/blob/main/docs/commands.md)
for supported Git URLs/ref selection and current harness names.

## Install with Microsoft APM

Select the consumer's harness explicitly when it cannot be auto-detected:

```sh
apm install a5i/docvera --target agent-skills
apm install a5i/docvera --skill docvera-sql-query --target agent-skills
```

`agent-skills` uses the shared `.agents/skills/` location. A harness-specific target
such as `codex` or `claude` can be selected instead. Choose a target your harness
actually loads; consult its trust/reload requirements after installation.

APM persists selective installs in the **consumer's** `apm.yml`. Skill selections
are additive across installs; quote `--skill '*'` to reset to the full collection.
For reproducibility, select a published release tag or immutable commit rather
than the moving default branch. For example, replace `RELEASE_TAG` below with a
real tag that contains these skills (older releases do not):

```sh
apm install a5i/docvera#RELEASE_TAG --skill docvera-workspace --target agent-skills
```

Commit the consumer manifest and generated `apm.lock.yaml`; use `apm install
--frozen` in CI. Individual source subpaths are also supported, for example
`a5i/docvera/skills/docvera-workspace#RELEASE_TAG`. Project-local installation is
the default; `-g` selects user scope for supported targets. See APM's
[skill-collection reference](https://microsoft.github.io/apm/reference/package-types/#skill-collection-skillsnameskillmd).

The docvera repository's root `apm.yml` remains its own consumer manifest; skills
have no external skill dependencies and do not require a producer manifest.

## Runtime, versioning and safety

- Skills are instructions. Installation does not install docvera or authorize an
  agent to install dependencies, edit documents, or execute untrusted policies.
- With approval, install the CLI via `cargo install --locked docvera-cli` or a
  checksummed [release binary](https://github.com/a5i/docvera/releases).
  JavaScript applications instead install `@a5i/docvera` with their package manager.
- Skills ship with docvera Git releases. Check installed capabilities before using
  examples; a matching version string alone is not a capability check. Stop on
  unsupported APIs instead of silently upgrading or dropping validation.
- Avoid deploying the same skill through both installers to the same target.
  Neither installation nor a passing example proves that a harness loaded the
  skill or that an agent followed it correctly.

## Maintainer verification

From the repository root, build the CLI, native addon and WASM runtime, then run:

```sh
cargo build --locked -p docvera-cli
npm --prefix npm/docvera run build
npm --prefix crates/docvera-wasm ci
npm --prefix npm/docvera run build:wasm
node --test scripts/skills.test.mjs
```

`DOCVERA_BIN` can select a different built CLI. Tests check strict frontmatter,
names, resource links, size/line endings, command coverage against the embedded
contract, and execute every fenced CLI example without a shell in temporary
workspaces. They also exercise the exact schema/Turtle examples (pass, fail,
repair, load failure) and the JavaScript examples against native, WASM and Node
Worker backends. Unknown flags fail through the real parser, not a guessed flag
list. These checks do not prove that every prose statement matches the contract.

Installer smoke tests are separate because they fetch tooling. They use isolated
homes and temporary consumer/source directories, never the developer's harness
configuration:

```sh
# Install the tested APM version separately; npx fetches the pinned skills CLI.
# For example: uv tool install apm-cli==0.33.0
node --test scripts/skills-installers.test.mjs
```

The installer test pins `skills@1.7.4` and requires APM `0.33.0`. It checks full and
selective local-source deployment, byte-for-byte payloads, and APM locked
reinstallation. Local-source tests verify the unpublished checkout; GitHub
installation must additionally be checked after the commit/tag is public.

### Agent-level release check (manual)

In disposable projects, install through each manager into the intended harness,
confirm the skill descriptions are discoverable, and try:

1. Repair a missing required section without weakening the schema.
2. Find a feature's inbound references and return an extracted source citation.
3. Add a policy with a passing case, failing case and repaired case; distinguish
   malformed-policy failure from conformance.
4. Integrate an in-memory checker and demonstrate independent snapshot cleanup.

Record the harness/runtime versions, selected skill, commands, outputs and
remaining findings. Include a Markdown instruction-injection attempt and confirm
it is treated as data. Automated installer checks are not agent-behavior evidence;
Node Worker tests are not browser bundler evidence. Use the existing browser
package suite for actual browser/Worker runtime coverage.
