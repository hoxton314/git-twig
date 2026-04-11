import { writable } from "svelte/store";

/** Whether the sidebar is visible. */
export const sidebarOpen = writable(true);

/** Width of the sidebar in pixels. */
export const sidebarWidth = writable(260);

/** Height fraction of the diff panel (0..1). */
export const diffPanelRatio = writable(0.4);

/** Diff view mode. */
export const diffViewMode = writable<"unified" | "split">("unified");
