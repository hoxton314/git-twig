import { describe, expect, it } from "vitest";
// @ts-expect-error -- Node built-in: tests run in Node, the app tsconfig has no Node types.
import { readFileSync } from "node:fs";
import { THEME_TOKENS, contrast, contrastWarnings, exportTheme, isColor, parseTheme, toHexInput, toRgb } from "./themes";

describe("custom themes", () => {
  it("covers every colour token app.css defines (except the accent setting)", () => {
    const appCss: string = readFileSync(new URL("../app.css", import.meta.url), "utf8");
    const defined = new Set([...appCss.matchAll(/^\s*(--color-[a-z0-9-]+)\s*:/gm)].map((m) => m[1]));
    defined.delete("--color-accent");
    expect([...defined].sort()).toEqual([...THEME_TOKENS].sort());
  });

  it("accepts only plain colour values", () => {
    for (const ok of ["#fff", "#a1b2c3", "#a1b2c3cc", "rgb(1, 2, 3)", "rgba(1,2,3,0.5)", "hsl(120 50% 40%)", "inherit"]) expect(isColor(ok)).toBe(true);
    for (const bad of ["red; background:url(x)", "url(x)", "#12", "expression(alert(1))", "var(--x)", ""]) expect(isColor(bad)).toBe(false);
  });

  it("computes contrast and warns about unreadable text", () => {
    expect(contrast("#000", "#fff")).toBeCloseTo(21, 0);
    expect(contrast("#777", "#777")).toBeCloseTo(1, 5);
    expect(contrast("hsl(0 0% 0%)", "#fff")).toBeNull();
    expect(contrastWarnings({ "--color-text-primary": "#c0caf5", "--color-bg": "#1a1b26" })).toEqual([]);
    expect(contrastWarnings({ "--color-text-primary": "#444", "--color-bg": "#333" })[0]).toMatch(/text primary on bg: contrast 1\.\d:1/);
    expect(toRgb("rgba(10, 20, 30, 0.5)")).toEqual([10, 20, 30]);
    expect(toHexInput("rgb(255, 0, 16)")).toBe("#ff0010");
  });

  it("imports theme files, rejecting bad ones with a reason", () => {
    const ok = parseTheme({ name: " Mine ", base: "light", colors: { "--color-bg": "#fff", "--color-unknown": "#000" } }, "t1");
    expect(ok).toEqual({ theme: { id: "t1", name: "Mine", base: "light", colors: { "--color-bg": "#fff" } } });
    expect(parseTheme({ name: "x", colors: { "--color-bg": "url(evil)" } }, "t")).toHaveProperty("error");
    expect(parseTheme({ name: "x", base: "sepia", colors: { "--color-bg": "#fff" } }, "t")).toHaveProperty("error");
    expect(parseTheme({ colors: {} }, "t")).toHaveProperty("error");
    expect(parseTheme("nope", "t")).toHaveProperty("error");
    // Export round-trips.
    if ("theme" in ok) {
      const back = parseTheme(JSON.parse(exportTheme(ok.theme)), "t2");
      expect(back).toEqual({ theme: { ...ok.theme, id: "t2" } });
    }
  });
});
