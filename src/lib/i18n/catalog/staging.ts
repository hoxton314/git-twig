/** Staging area, changed files, stash, co-authors. */
import type { Source, Translation } from "../types";

export const en = {} as const satisfies Source;

export const pl: Translation<typeof en> = {};
