import { describe, expect, it } from "vitest";
import { buildEntries, filterEntries, flatRows, stepSelection, firstSelectable, highlightRuns } from "./repoMenu";

const quick = [
  { path: "/code/twig", name: "twig", favorite: true, missing: false },
  { path: "/code/old-thing", name: "old-thing", favorite: false, missing: true },
  { path: "/code/notes", name: "notes", favorite: false, missing: false },
];
const scanned = [
  { path: "/code/twig", name: "twig", head_name: "main" }, // duplicate of a favorite
  { path: "/code/tauri-plugin", name: "tauri-plugin", head_name: "feature/window" },
  { path: "/code/website", name: "website", head_name: "main" },
];

describe("repo menu", () => {
  const entries = buildEntries(quick, scanned, new Set(["/code/website"]));

  it("merges both sections without duplicate paths", () => {
    expect(entries.map((e) => `${e.kind}:${e.name}`)).toEqual([
      "favorite:twig",
      "recent:old-thing",
      "recent:notes",
      "scanned:tauri-plugin",
      "scanned:website",
    ]);
    expect(entries.find((e) => e.name === "website")?.open).toBe(true);
  });

  it("keeps everything, in order, without a query", () => {
    const sections = filterEntries(entries, "  ");
    expect(sections.map((s) => s.title)).toEqual(["Favorites & recent", "Repositories"]);
    expect(flatRows(sections).map((r) => r.name)).toEqual(["twig", "old-thing", "notes", "tauri-plugin", "website"]);
  });

  it("fuzzy-filters across both sections by name, branch and path", () => {
    const names = (q: string) => flatRows(filterEntries(entries, q)).map((r) => r.name);
    expect(names("twi")).toEqual(["twig"]);
    expect(names("tpl")).toEqual(["tauri-plugin"]);
    expect(names("feature/win")).toEqual(["tauri-plugin"]); // branch
    expect(names("old")).toEqual(["old-thing"]); // missing repos stay filterable
    expect(names("zzz")).toEqual([]);
    // Empty sections are dropped.
    expect(filterEntries(entries, "tauri").map((s) => s.title)).toEqual(["Repositories"]);
  });

  it("highlights the matched name characters", () => {
    const [row] = flatRows(filterEntries(entries, "tpl"));
    expect(highlightRuns(row.name, row.nameHits)).toEqual([
      { text: "t", hit: true },
      { text: "auri-", hit: false },
      { text: "pl", hit: true },
      { text: "ugin", hit: false },
    ]);
  });

  it("keyboard selection skips missing repos and wraps", () => {
    const rows = flatRows(filterEntries(entries, ""));
    expect(firstSelectable(rows)).toBe(0);
    expect(stepSelection(rows, 0, 1)).toBe(2); // skips old-thing
    expect(stepSelection(rows, 2, -1)).toBe(0);
    expect(stepSelection(rows, 0, -1)).toBe(4); // wraps to the end
    const onlyMissing = flatRows(filterEntries(entries, "old"));
    expect(firstSelectable(onlyMissing)).toBe(-1);
    expect(stepSelection(onlyMissing, -1, 1)).toBe(-1);
  });

  it("stays fast with hundreds of repositories", () => {
    const many = Array.from({ length: 800 }, (_, i) => ({ path: `/r/${i}/project-${i}`, name: `project-${i}`, head_name: "main" }));
    const big = buildEntries([], many, new Set());
    const t = performance.now();
    for (let k = 0; k < 20; k++) filterEntries(big, `pro${k}`);
    expect((performance.now() - t) / 20).toBeLessThan(25); // ms per keystroke
  });
});
