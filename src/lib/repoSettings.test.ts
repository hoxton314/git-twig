import { describe, expect, it } from "vitest";
import { effectiveSettings, isOverridable, sanitizeOverride, splitPatch } from "./repoSettings";
import type { AppSettings } from "./types/git";

const global = {
  auto_fetch_interval: 0,
  max_commits: 5000,
  context_lines: 3,
  graph_hide_remotes: false,
  external_diff_tool: null,
  theme: "dark",
} as unknown as AppSettings;

describe("per-repository settings", () => {
  it("applies a repository's overrides only to that repository", () => {
    const overrides = { "/a": { context_lines: 10 } };
    expect(effectiveSettings(global, overrides, "/a").context_lines).toBe(10);
    expect(effectiveSettings(global, overrides, "/b").context_lines).toBe(3);
    expect(effectiveSettings(global, overrides, null)).toBe(global);
  });

  it("drops unknown keys and wrongly typed values", () => {
    expect(
      sanitizeOverride({ context_lines: 5, theme: "light", max_commits: "lots", external_diff_tool: "meld", nope: 1 }, global),
    ).toEqual({ context_lines: 5, external_diff_tool: "meld" });
    expect(sanitizeOverride(null, global)).toEqual({});
    expect(isOverridable("theme")).toBe(false);
  });

  it("routes changes to the override when the repository overrides that key", () => {
    const overrides = { "/a": { graph_hide_remotes: true } };
    expect(splitPatch({ graph_hide_remotes: false, theme: "light" } as Partial<AppSettings>, overrides, "/a")).toEqual({
      global: { theme: "light" },
      repo: { graph_hide_remotes: false },
    });
    expect(splitPatch({ graph_hide_remotes: false }, overrides, "/b")).toEqual({
      global: { graph_hide_remotes: false },
      repo: {},
    });
  });
});
