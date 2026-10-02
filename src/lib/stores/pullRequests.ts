import { writable } from "svelte/store";

/** Whether the Pull Requests panel (modal) is open. */
export const prPanelOpen = writable(false);

/** Open the Pull Requests panel. */
export function openPullRequests() {
  prPanelOpen.set(true);
}
