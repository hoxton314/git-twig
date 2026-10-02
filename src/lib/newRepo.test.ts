import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { cloneFolderName, joinPath, newRepoDialog, openNewRepoDialog } from "./newRepo";

describe("cloneFolderName", () => {
  it("matches git's default folder name", () => {
    expect(cloneFolderName("https://github.com/hoxton314/git-twig.git")).toBe("git-twig");
    expect(cloneFolderName("https://gitlab.com/group/sub/proj")).toBe("proj");
    expect(cloneFolderName("git@github.com:owner/repo.git")).toBe("repo");
    expect(cloneFolderName("ssh://git@host:2222/owner/repo.git/")).toBe("repo");
    expect(cloneFolderName("/srv/git/thing.git/")).toBe("thing");
    expect(cloneFolderName("file:///C:/repos/x.git")).toBe("x");
  });

  it("never produces path separators or a leading dot/dash", () => {
    expect(cloneFolderName("https://h/o/..")).not.toMatch(/[\\/]|^[.-]/);
    expect(cloneFolderName("https://h/o/-evil")).toBe("evil");
    expect(cloneFolderName("")).toBe("");
  });
});

describe("joinPath", () => {
  it("uses the folder's separator", () => {
    expect(joinPath("/home/me/code/", "repo")).toBe("/home/me/code/repo");
    expect(joinPath("C:\\Users\\me", "repo")).toBe("C:\\Users\\me\\repo");
  });
});

describe("openNewRepoDialog", () => {
  it("does not switch a dialog that is already open (e.g. mid-clone)", () => {
    newRepoDialog.set(null);
    openNewRepoDialog("clone");
    openNewRepoDialog("init");
    expect(get(newRepoDialog)).toBe("clone");
    newRepoDialog.set(null);
    openNewRepoDialog("init");
    expect(get(newRepoDialog)).toBe("init");
  });
});
