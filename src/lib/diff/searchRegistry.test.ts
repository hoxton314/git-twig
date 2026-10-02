import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { DiffSearchRegistry } from "./searchRegistry";

/** Fake elements ordered by `pos` (what compareDocumentPosition reports). */
function el(pos: number): Element {
  return {
    pos,
    compareDocumentPosition(other: { pos: number }) {
      return other.pos > pos ? 4 : 2; // FOLLOWING : PRECEDING
    },
  } as unknown as Element;
}

describe("DiffSearchRegistry", () => {
  it("orders hunks by document position, not registration order", () => {
    const r = new DiffSearchRegistry();
    const a = el(1), b = el(2), c = el(3);
    r.register("c", c, ["c:1"]);
    r.register("a", a, ["a:1", "a:2"]);
    r.register("b", b, []);
    r.register("b2", el(2.5), ["b2:1"]);
    expect(r.orderedIds()).toEqual(["a:1", "a:2", "b2:1", "c:1"]);
  });

  it("re-registration replaces ids; a stale unregister is ignored", () => {
    const r = new DiffSearchRegistry();
    const old = el(1), fresh = el(1);
    r.register("h", old, ["h:1"]);
    r.register("h", fresh, ["h:1", "h:2"]);
    r.unregister("h", old); // the old instance's cleanup runs late
    expect(r.orderedIds()).toEqual(["h:1", "h:2"]);
    const v = get(r.version);
    r.unregister("h", fresh);
    expect(r.orderedIds()).toEqual([]);
    expect(get(r.version)).toBe(v + 1);
  });
});
