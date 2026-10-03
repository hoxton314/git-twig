import { describe, expect, it, vi } from "vitest";

vi.mock("./tauri", () => ({}));
vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-notification", () => ({}));
vi.mock("./stores/settings", async () => ({ globalSettings: (await import("svelte/store")).writable({ notify_ci: "all" }) }));
vi.mock("./stores/toasts", () => ({ toast: vi.fn() }));

const { ciTransition, runUrl } = await import("./ciWatch");

describe("CI finished notifications", () => {
  it("announces only pending → finished transitions, per mode", () => {
    expect(ciTransition("pending", "success", "all")).toBe("success");
    expect(ciTransition("pending", "failure", "all")).toBe("failure");
    expect(ciTransition("pending", "success", "failures")).toBeNull();
    expect(ciTransition("pending", "failure", "failures")).toBe("failure");
    expect(ciTransition("pending", "failure", "off")).toBeNull();
    // Already finished when first seen, or still running: nothing to say.
    expect(ciTransition(undefined, "failure", "all")).toBeNull();
    expect(ciTransition("success", "success", "all")).toBeNull();
    expect(ciTransition("pending", "pending", "all")).toBeNull();
    expect(ciTransition("pending", undefined, "all")).toBeNull();
  });

  it("links the failing check's run", () => {
    const check = (name: string, state: "success" | "failure", url: string | null) => ({ name, state, description: null, url });
    expect(runUrl({ sha: "s", state: "failure", checks: [check("a", "success", "u1"), check("b", "failure", "u2")] })).toBe("u2");
    expect(runUrl({ sha: "s", state: "success", checks: [check("a", "success", "u1")] })).toBe("u1");
    expect(runUrl({ sha: "s", state: "success", checks: [] })).toBeNull();
  });
});
