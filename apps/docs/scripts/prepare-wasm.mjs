import { cp, mkdir, readFile, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
let root;
try {
  root = dirname(require.resolve("@a5i/docvera/package.json"));
} catch (error) {
  if (error.code !== "MODULE_NOT_FOUND") throw error;
  console.error("Missing required dependency @a5i/docvera.\nRun `npm ci --include=dev` in apps/docs, then retry `npm run dev`.");
  process.exit(1);
}
const manifest = JSON.parse(await readFile(join(root, "package.json"), "utf8"));
if (manifest.version !== "0.2.0") throw new Error("Review WASM fixtures before upgrading docvera.");
const destination = new URL("../public/runtime/", import.meta.url);
await rm(destination, { recursive: true, force: true });
await mkdir(destination, { recursive: true });
for (const file of ["index.mjs", "core.mjs", "worker-client.mjs", "worker.mjs", "web"]) {
  await cp(join(root, "wasm", file), new URL(file, destination), { recursive: true });
}
console.log(`Prepared docvera ${manifest.version} browser runtime (no native addon).`);
