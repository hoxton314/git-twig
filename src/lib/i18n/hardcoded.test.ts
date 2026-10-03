import { describe, expect, it } from "vitest";
import { findHardcoded, isTranslatable } from "./hardcoded";

const components = import.meta.glob("../../components/**/*.svelte", { query: "?raw", import: "default", eager: true }) as Record<
  string,
  string
>;

describe("hard-coded strings", () => {
  it("finds text, attributes, expression literals and toasts", () => {
    const src = [
      "<script>",
      '  toast("error", "Push failed");',
      '  toastError("Fetch failed", e);',
      '  const opts = [{ value: 1, label: "One minute" }];',
      '  const cls = "row active";',
      "</script>",
      '<button title="Delete branch" class="btn">Delete</button>',
      '<span>{busy ? "Working…" : $t("x.done")}</span>',
      "<code>HEAD</code> <kbd>Ctrl+K</kbd> {count}",
      '<span title="Skip">{"skip"}</span> <!-- i18n-ignore --> <b>Kept</b>',
      "<style>.a::after { content: 'Not text'; }</style>",
    ].join("\n");
    expect(findHardcoded(src)).toEqual([
      { line: 2, text: "Push failed" },
      { line: 3, text: "Fetch failed" },
      { line: 4, text: "One minute" },
      { line: 7, text: "Delete branch" },
      { line: 7, text: "Delete" },
      { line: 8, text: "Working…" },
    ]);
  });

  it("allows product names, git terms and symbols", () => {
    expect(isTranslatable("GitHub")).toBe(false);
    expect(isTranslatable("HEAD → {branch}")).toBe(false);
    expect(isTranslatable("Ctrl+Shift+P")).toBe(false);
    expect(isTranslatable("(+{count})")).toBe(false);
    expect(isTranslatable("Open")).toBe(true);
  });

  it("no component has user-visible text outside the message catalog", () => {
    expect(Object.keys(components).length).toBeGreaterThan(50);
    const report = Object.entries(components)
      .flatMap(([file, src]) => findHardcoded(src).map((f) => `${file.replace("../../", "src/")}:${f.line}  ${f.text}`))
      .sort();
    // Translate with $t("area.key") / tr(); mark a line `i18n-ignore` only for text that is not UI language.
    expect(report).toEqual([]);
  });
});
