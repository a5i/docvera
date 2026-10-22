import { expect, test } from "@playwright/test";

for (const route of ["/", "/docs/", "/docs/quickstart/", "/docs/schemas/", "/docs/sql/", "/docs/wasm/", "/playground/"]) {
  test(`renders ${route} without errors or horizontal overflow`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.goto(route);
    await expect(page.locator("main h1")).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    expect(errors).toEqual([]);
  });
}

test("real WASM validates, reports invalid metadata, resets and queries SQL", async ({ page }) => {
  await page.goto("/playground/");
  await page.getByRole("button", { name: "Run check", exact: true }).click();
  await expect(page.getByText("All checks passed")).toBeVisible({ timeout: 25_000 });
  await page.getByRole("button", { name: "Introduce an error" }).click();
  await expect(page.getByText("Changes not checked")).toBeVisible();
  await page.getByRole("button", { name: "Run check", exact: true }).click();
  await expect(page.locator(".diagnostic")).not.toHaveCount(0);
  await expect(page.locator(".diagnostic-summary")).toContainText("1 errors");
  await page.getByRole("button", { name: "Reset example" }).click();
  await page.getByRole("tab", { name: "query.sql" }).click();
  await page.getByRole("button", { name: "Run query", exact: true }).click();
  await expect(page.locator(".query-table")).toContainText("NOTE-001", { timeout: 25_000 });
  await expect(page.locator(".query-table")).toContainText("docs/note.md");
});

test("SQL errors can be corrected and rerun", async ({ page }) => {
  await page.goto("/playground/");
  await page.getByRole("tab", { name: "query.sql" }).click();
  const editor = page.locator(".cm-content");
  await editor.fill("SELECT * FROM missing_table");
  await page.getByRole("button", { name: "Run query", exact: true }).click();
  await expect(page.getByText("Could not run this workspace")).toBeVisible({ timeout: 25_000 });
  await editor.fill("SELECT COUNT(*) AS n FROM documents");
  await page.getByRole("button", { name: "Run query", exact: true }).click();
  await expect(page.locator(".query-table td")).toHaveText("1", { timeout: 25_000 });
});

test("day and night themes persist and produce screenshots", async ({ page }, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await page.goto("/");
  await page.evaluate(() => document.fonts.ready);
  await expect(page.locator("html")).toHaveClass(/light/);
  const toolbar = await page.locator(".editor-toolbar").boundingBox();
  const editor = await page.locator(".editor-area:visible").boundingBox();
  const outputBar = await page.locator(".output-bar").boundingBox();
  const output = await page.locator(".output-body").boundingBox();
  expect(toolbar && editor && editor.y >= toolbar.y + toolbar.height - 1).toBeTruthy();
  expect(outputBar && output && output.y >= outputBar.y + outputBar.height - 1).toBeTruthy();
  await page.screenshot({ path: testInfo.outputPath("home-day.png"), fullPage: true });
  await page.getByRole("button", { name: "Toggle day / night theme" }).click();
  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.reload();
  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.screenshot({ path: testInfo.outputPath("home-night.png"), fullPage: true });
});

test("documentation search and heading anchors work", async ({ page }) => {
  await page.goto("/docs/quickstart/");
  await page.getByRole("button", { name: "Search documentation", exact: true }).click();
  await page.getByRole("textbox", { name: "Search documentation" }).fill("SQL");
  await page.getByRole("dialog").getByRole("link", { name: /Query with SQL/ }).click();
  await expect(page).toHaveURL(/\/docs\/sql\//);
  await expect(page.getByRole("heading", { name: "Query with SQL", exact: true })).toBeVisible();
  await expect(page.locator("#find-your-documents")).toHaveCount(1);
});


test("file tabs work with the keyboard and edited schemas are executed", async ({ page }) => {
  await page.goto("/playground/");
  const documentTab = page.getByRole("tab", { name: "note.md", exact: true });
  await documentTab.click();
  await documentTab.focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "note.yaml", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("tab", { name: "note.yaml", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tabpanel", { name: "note.yaml", exact: true })).toBeVisible();
  const editor = page.locator(".cm-content");
  await editor.fill("schemaVersion: 1\ndocumentType: note\nfrontMatter:\n  type: object\n  required: [owner]\nbody: {allowPreamble: true, additionalSections: true}\n");
  await page.getByRole("button", { name: "Run check", exact: true }).click();
  await expect(page.locator(".diagnostic")).toContainText("owner", { timeout: 25_000 });
});

test("search shortcut opens after hydration and Escape restores focus", async ({ page }) => {
  await page.goto("/");
  const search = page.getByRole("button", { name: "Search documentation", exact: true });
  await search.click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(search).toBeFocused();
  await page.keyboard.press("Control+k");
  await expect(page.getByRole("textbox", { name: "Search documentation" })).toBeFocused();
});


const scenarios = [
  { id: "metadata", documents: 1, rows: 1, text: "NOTE-001" },
  { id: "structure", documents: 1, rows: 4, text: "Consequences" },
  { id: "links", documents: 2, rows: 2, text: "docs/related.md" },
  { id: "identity", documents: 2, rows: 2, text: "NOTE-002" },
  { id: "sections-sql", documents: 2, rows: 4, text: "ARCH-001" },
  { id: "workspace-report", documents: 3, rows: 1, text: "3" },
];

for (const scenario of scenarios) {
  test(`${scenario.id}: valid workspace, real SQL, characteristic validation error and reset`, async ({ page }, testInfo) => {
    await page.goto("/playground/");
    await page.getByRole("combobox", { name: "Choose example" }).selectOption(scenario.id);
    await page.getByRole("button", { name: "Run check", exact: true }).click();
    await expect(page.getByText("All checks passed")).toBeVisible({ timeout: 25_000 });
    await expect(page.locator(".success-result")).toContainText(`${scenario.documents} ${scenario.documents === 1 ? "document" : "documents"} validated`);
    await page.getByRole("tab", { name: "query.sql", exact: true }).click();
    await page.getByRole("button", { name: "Run query", exact: true }).click();
    await expect(page.locator(".query-table tbody tr")).toHaveCount(scenario.rows, { timeout: 25_000 });
    await expect(page.locator(".query-table")).toContainText(scenario.text);
    await page.getByRole("button", { name: "Introduce an error" }).click();
    await page.getByRole("button", { name: "Run check", exact: true }).click();
    await expect(page.locator(".diagnostic").first()).toBeVisible({ timeout: 25_000 });
    await expect(page.locator(".diagnostic-summary")).not.toContainText("0 errors");
    await page.getByRole("button", { name: "Reset example" }).click();
    await expect(page.getByRole("combobox", { name: "Choose example" })).toHaveValue(scenario.id);
    await page.getByRole("button", { name: "Run check", exact: true }).click();
    await expect(page.getByText("All checks passed")).toBeVisible({ timeout: 25_000 });
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    if (scenario.id === "links") {
      await page.evaluate(() => {
        if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
        window.scrollTo({ top: 0, behavior: "instant" });
      });
      await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
      await page.screenshot({ path: testInfo.outputPath("multi-document-example.png"), fullPage: true });
    }
  });
}

test("secondary files retain edits and checked edits are protected when switching examples", async ({ page }) => {
  await page.goto("/playground/");
  const examples = page.getByRole("combobox", { name: "Choose example" });
  await examples.selectOption("links");
  const files = page.getByRole("combobox", { name: "Workspace document" });
  await files.selectOption("docs/related.md");
  const editor = page.locator(".cm-content");
  await expect(editor).toContainText("A shared decision");
  await editor.fill("---\nid: NOTE-002\ntitle: Edited note\nstatus: accepted\n---\n# Edited note\n\n## Context\nChanged in the second document.\n\n## Decision\nPreserve edits while switching files.\n");
  await files.selectOption("docs/overview.md");
  await expect(editor).toContainText("Connected knowledge");
  await files.selectOption("docs/related.md");
  await expect(editor).toContainText("Edited note");
  await page.getByRole("button", { name: "Run check", exact: true }).click();
  await expect(page.getByText("All checks passed")).toBeVisible({ timeout: 25_000 });
  page.once("dialog", dialog => dialog.dismiss());
  await examples.selectOption("identity");
  await expect(examples).toHaveValue("links");
  await expect(editor).toContainText("Edited note");
  page.once("dialog", dialog => dialog.accept());
  await examples.selectOption("identity");
  await expect(examples).toHaveValue("identity");
  await expect(editor).toContainText("First decision");
  await expect(page.locator(".query-table")).toHaveCount(0);
});
