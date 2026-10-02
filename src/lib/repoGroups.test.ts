import { describe, expect, it } from "vitest";
import { addToGroup, createGroup, deleteGroup, groupsOf, newGroupId, removeFromGroup, renameGroup, uniqueGroupName } from "./repoGroups";

describe("repository groups", () => {
  it("creates groups with unique names and ids", () => {
    let gs = createGroup([], "a", "Work", ["/x", "/x", "/y"]);
    expect(gs).toEqual([{ id: "a", name: "Work", paths: ["/x", "/y"] }]);
    gs = createGroup(gs, "b", "work");
    expect(gs[1].name).toBe("work 2");
    expect(uniqueGroupName(gs, "  ")).toBe("Group");
    expect(gs.map((g) => g.id)).not.toContain(newGroupId(gs));
    // Never longer than the backend keeps, so names survive a restart unchanged.
    const long = "x".repeat(150);
    gs = createGroup(gs, "c", long);
    gs = createGroup(gs, "d", long);
    expect(gs[2].name.length).toBeLessThanOrEqual(100);
    expect(gs[3].name.length).toBeLessThanOrEqual(100);
    expect(gs[3].name).not.toBe(gs[2].name);
  });

  it("renames, adds, removes and deletes", () => {
    let gs = createGroup(createGroup([], "a", "Work"), "b", "OSS");
    gs = renameGroup(gs, "b", "Work");
    expect(gs[1].name).toBe("Work 2");
    expect(renameGroup(gs, "a", "   ")).toBe(gs);
    gs = addToGroup(gs, "a", "/x");
    gs = addToGroup(gs, "a", "/x");
    gs = addToGroup(gs, "b", "/x");
    expect(gs[0].paths).toEqual(["/x"]);
    expect(groupsOf(gs, "/x").map((g) => g.id)).toEqual(["a", "b"]);
    gs = removeFromGroup(gs, "a", "/x");
    expect(groupsOf(gs, "/x").map((g) => g.id)).toEqual(["b"]);
    expect(deleteGroup(gs, "b").map((g) => g.id)).toEqual(["a"]);
  });
});
