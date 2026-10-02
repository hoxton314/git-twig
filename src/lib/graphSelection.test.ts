import { describe, expect, it } from "vitest";
import { EMPTY_SELECTION, comparePair, effectiveSelection, extendTo, primaryAfterToggle, toggle } from "./graphSelection";

const order = ["e", "d", "c", "b", "a"]; // newest first

describe("graph multi-selection", () => {
  it("collapses to the primary when it is not in the set", () => {
    expect(effectiveSelection(EMPTY_SELECTION, null)).toEqual([]);
    expect(effectiveSelection(EMPTY_SELECTION, "c")).toEqual(["c"]);
    expect(effectiveSelection({ oids: ["d", "b"], anchor: "b" }, "b")).toEqual(["d", "b"]);
    // Something else selected a different commit: the set no longer applies.
    expect(effectiveSelection({ oids: ["d", "b"], anchor: "b" }, "a")).toEqual(["a"]);
    expect(effectiveSelection({ oids: ["d", "b"], anchor: "b" }, "__wip__")).toEqual([]);
  });

  it("toggles commits in graph order", () => {
    let sel = toggle(EMPTY_SELECTION, "b", "d", order);
    expect(sel).toEqual({ oids: ["d", "b"], anchor: "d" });
    expect(primaryAfterToggle(sel, "d")).toBe("d");
    sel = toggle(sel, "d", "a", order);
    expect(sel.oids).toEqual(["d", "b", "a"]);
    sel = toggle(sel, "a", "d", order);
    expect(sel.oids).toEqual(["b", "a"]);
    expect(primaryAfterToggle(sel, "d")).toBe("b");
    // Toggling the only commit off leaves nothing.
    const none = toggle(EMPTY_SELECTION, "c", "c", order);
    expect(none.oids).toEqual([]);
    expect(primaryAfterToggle(none, "c")).toBeNull();
  });

  it("extends a range from the anchor in either direction", () => {
    expect(extendTo(EMPTY_SELECTION, "d", "b", order)).toEqual({ oids: ["d", "c", "b"], anchor: "d" });
    const sel = extendTo(EMPTY_SELECTION, "b", "e", order);
    expect(sel.oids).toEqual(["e", "d", "c", "b"]);
    // Re-extending keeps the same anchor (shrinks the range).
    expect(extendTo(sel, "e", "c", order).oids).toEqual(["c", "b"]);
    // An anchor no longer in the graph falls back to the primary.
    expect(extendTo({ oids: [], anchor: "gone" }, "a", "b", order).oids).toEqual(["b", "a"]);
    // Nothing to anchor on: just the target.
    expect(extendTo(EMPTY_SELECTION, null, "c", order)).toEqual({ oids: ["c"], anchor: "c" });
  });

  it("compares older → newer for exactly two commits", () => {
    expect(comparePair(["d", "b"])).toEqual({ from: "b", to: "d" });
    expect(comparePair(["d"])).toBeNull();
    expect(comparePair(["e", "d", "c"])).toBeNull();
  });
});
