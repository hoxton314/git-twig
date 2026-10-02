/**
 * Typed wrappers for all Tauri invoke() calls, one function per command.
 * No direct invoke() elsewhere in the frontend. Implementations live in
 * `src/lib/api/<area>.ts`; import from here (`import * as tauri`).
 */
export * from "./api/repo";
export * from "./api/graph";
export * from "./api/branches";
export * from "./api/diff";
export * from "./api/stash";
export * from "./api/settings";
export * from "./api/github";
export * from "./api/history";
export * from "./api/workspace";
export * from "./api/hosting";
export * from "./api/lfs";
