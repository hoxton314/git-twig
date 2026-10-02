import { describe, expect, it } from "vitest";
import { matchRanges, parsePaths, splitRuns, workingArea } from "./codeSearch";

const plain = { regex: false, ignoreCase: false, wholeWord: false };

describe("code search helpers", () => {
  it("opens changed files as diffs, unchanged ones in blame", () => {
    const status = { staged: [{ path: "new.rs" }, { path: "both.rs" }], unstaged: [{ path: "both.rs" }, { path: "edit.rs" }] };
    expect(workingArea("edit.rs", status)).toBe("unstaged");
    expect(workingArea("both.rs", status)).toBe("unstaged");
    expect(workingArea("new.rs", status)).toBe("staged");
    expect(workingArea("clean.rs", status)).toBeNull();
  });

  it("splits the path filter", () => {
    expect(parsePaths(" src/, *.rs  docs ")).toEqual(["src/", "*.rs", "docs"]);
    expect(parsePaths("")).toEqual([]);
  });

  it("finds literal, case-insensitive, whole-word and regex matches", () => {
    expect(matchRanges("a.b a.b", "a.b", plain)).toEqual([[0, 3], [4, 7]]);
    expect(matchRanges("axb", "a.b", plain)).toEqual([]);
    expect(matchRanges("Main main", "main", { ...plain, ignoreCase: true })).toEqual([[0, 4], [5, 9]]);
    expect(matchRanges("domain main", "main", { ...plain, wholeWord: true })).toEqual([[7, 11]]);
    expect(matchRanges("fn Foo()", "^fn [A-Z]", { ...plain, regex: true })).toEqual([[0, 4]]);
    // Invalid in JS: no highlight rather than an error.
    expect(matchRanges("x", "(", { ...plain, regex: true })).toEqual([]);
    expect(matchRanges("x", "", plain)).toEqual([]);
  });

  it("splits text into highlighted runs", () => {
    expect(splitRuns("let main = 1", [[4, 8]])).toEqual([
      { text: "let ", hit: false },
      { text: "main", hit: true },
      { text: " = 1", hit: false },
    ]);
    expect(splitRuns("abc", [])).toEqual([{ text: "abc", hit: false }]);
    expect(splitRuns("ab", [[0, 2]])).toEqual([{ text: "ab", hit: true }]);
  });
});
