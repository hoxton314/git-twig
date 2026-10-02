import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type Kb = typeof import("./keybindings");
type Listener = (e: KeyboardEvent) => void;

let kb: Kb;
let listener: Listener;
let modalOpen = false;

function ev(o: Partial<KeyboardEvent> & { tag?: string }): KeyboardEvent {
  const { tag = "DIV", ...rest } = o;
  return {
    key: "x",
    code: "",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    isComposing: false,
    defaultPrevented: false,
    target: { tagName: tag, isContentEditable: false },
    preventDefault() {
      (this as { defaultPrevented: boolean }).defaultPrevented = true;
    },
    ...rest,
  } as unknown as KeyboardEvent;
}

const press = (o: Parameters<typeof ev>[0]) => listener(ev(o));
const ctrlShiftP = { key: "P", ctrlKey: true, shiftKey: true };

beforeEach(async () => {
  // The registry keeps module-level state: load a fresh copy per test.
  vi.resetModules();
  modalOpen = false;
  vi.stubGlobal("window", {
    addEventListener: (_type: string, fn: Listener) => {
      listener = fn;
    },
  });
  vi.stubGlobal("document", { querySelector: () => (modalOpen ? {} : null) });
  vi.stubGlobal("navigator", { platform: "Linux x86_64", userAgent: "" });
  kb = await import("./keybindings");
  kb.installKeybindings();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("dispatch", () => {
  it("runs the handler of the matching shortcut", () => {
    const push = vi.fn();
    kb.onAction("push", push);
    press(ctrlShiftP);
    expect(push).toHaveBeenCalledOnce();
  });

  it("does not let a conflicting binding without a handler swallow the key", () => {
    kb.setOverrides({ fetch: "Ctrl+Shift+P" }); // push (no handler) shares it
    const fetch = vi.fn();
    kb.onAction("fetch", fetch);
    press(ctrlShiftP);
    expect(fetch).toHaveBeenCalledOnce();
  });

  it("with two handlers on one shortcut, the first in ACTIONS wins", () => {
    kb.setOverrides({ fetch: "Ctrl+Shift+P" });
    const push = vi.fn();
    const fetch = vi.fn();
    kb.onAction("push", push);
    kb.onAction("fetch", fetch);
    press(ctrlShiftP);
    expect(push).toHaveBeenCalledOnce();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("is suppressed while a modal is open and during IME composition", () => {
    const push = vi.fn();
    kb.onAction("push", push);
    modalOpen = true;
    press(ctrlShiftP);
    modalOpen = false;
    press({ ...ctrlShiftP, isComposing: true });
    expect(push).not.toHaveBeenCalled();
  });

  it("only fires commit / allowInInputs actions inside text fields", () => {
    const commit = vi.fn();
    const sidebar = vi.fn();
    const palette = vi.fn();
    kb.onAction("commit", commit);
    kb.onAction("toggle_sidebar", sidebar);
    kb.onAction("command_palette", palette);
    press({ key: "Enter", ctrlKey: true, tag: "TEXTAREA" });
    press({ key: "b", ctrlKey: true, tag: "INPUT" });
    press({ key: "k", ctrlKey: true, tag: "INPUT" });
    expect(commit).toHaveBeenCalledOnce();
    expect(sidebar).not.toHaveBeenCalled();
    expect(palette).toHaveBeenCalledOnce();
  });

  it("keeps a newer registration when an older one unsubscribes", () => {
    const first = vi.fn();
    const second = vi.fn();
    const off = kb.onAction("push", first);
    kb.onAction("push", second);
    off();
    press(ctrlShiftP);
    expect(second).toHaveBeenCalledOnce();
    expect(first).not.toHaveBeenCalled();
  });
});

describe("overrides", () => {
  it("an empty override unbinds a default shortcut", () => {
    kb.setOverrides({ push: "" });
    const push = vi.fn();
    kb.onAction("push", push);
    press(ctrlShiftP);
    expect(push).not.toHaveBeenCalled();
    expect(kb.getShortcut("push")).toBe("");
    let labels: Record<string, string> = {};
    kb.shortcutLabels.subscribe((v) => (labels = v))();
    expect(labels.push).toBeUndefined();
  });

  it("resolveShortcut prefers a stored override, else the default", () => {
    expect(kb.resolveShortcut({}, "push")).toBe("Ctrl+Shift+P");
    expect(kb.resolveShortcut({ push: "Alt+P" }, "push")).toBe("Alt+P");
    expect(kb.resolveShortcut({ push: "" }, "push")).toBe("");
    expect(kb.resolveShortcut({}, "no_such_action")).toBe("");
  });
});

describe("shortcut parsing", () => {
  it("normalises modifier order, case and key aliases", () => {
    expect(kb.normalizeShortcut("Shift+Ctrl+p")).toBe("ctrl+shift+p");
    expect(kb.normalizeShortcut("Ctrl+Escape")).toBe(kb.normalizeShortcut("Ctrl+Esc"));
    expect(kb.normalizeShortcut("Ctrl++")).toBe("ctrl++");
    expect(kb.normalizeShortcut("Cmd+K")).toBe("ctrl+k");
  });

  it("matches the physical key on non-Latin layouts", () => {
    const push = vi.fn();
    kb.onAction("push", push);
    press({ key: "З", code: "KeyP", ctrlKey: true, shiftKey: true });
    expect(push).toHaveBeenCalledOnce();
  });

  it("turns key events into shortcut strings for the capture UI", () => {
    expect(kb.eventToShortcut(ev({ key: "p", ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+P");
    expect(kb.eventToShortcut(ev({ key: "Shift", shiftKey: true }))).toBeNull();
    expect(kb.eventToShortcut(ev({ key: " ", altKey: true }))).toBe("Alt+Space");
  });
});

describe("ACTIONS", () => {
  it("has unique ids and parseable default shortcuts", () => {
    const ids = kb.ACTIONS.map((a) => a.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const a of kb.ACTIONS) {
      if (a.defaultShortcut) expect(kb.normalizeShortcut(a.defaultShortcut)).not.toBe("");
    }
  });
});
