import { openChecker } from "./runtime/index.mjs";

self.onmessage = async ({ data }) => {
  let checker;
  try {
    checker = await openChecker({ config: data.config, sources: data.sources });
    const check = await checker.check();
    const query = data.sql ? await checker.querySql(data.sql) : null;
    self.postMessage({ ok: true, check, query });
  } catch (error) {
    self.postMessage({ ok: false, error: error instanceof Error ? error.message : String(error) });
  } finally {
    await checker?.close();
  }
};
