/**
 * In-diff search across windowed hunks.
 *
 * Hunks only render the rows near the viewport, so matches can't be counted
 * from the DOM. Instead each `DiffHunk` registers its match ids (computed
 * from data, in render order) together with its root element; the viewer
 * orders hunks by document position, counts, and publishes the active id.
 * The hunk that owns it highlights it and scrolls it into view.
 */
import { writable, type Writable } from "svelte/store";

export const DIFF_SEARCH_CONTEXT = "twig:diff-search";

interface Entry {
  el: Element;
  ids: string[];
}

export class DiffSearchRegistry {
  private entries = new Map<string, Entry>();
  /** Bumped on every (un)registration. */
  readonly version: Writable<number> = writable(0);
  /** Id of the active match (`null` when none). */
  readonly active: Writable<string | null> = writable(null);

  register(key: string, el: Element, ids: string[]) {
    this.entries.set(key, { el, ids });
    this.version.update((v) => v + 1);
  }

  unregister(key: string, el: Element) {
    // A newer registration under the same key may already have replaced it.
    if (this.entries.get(key)?.el !== el) return;
    this.entries.delete(key);
    this.version.update((v) => v + 1);
  }

  /** All match ids in document order. */
  orderedIds(): string[] {
    const list = [...this.entries.values()].filter((e) => e.ids.length > 0);
    list.sort((a, b) => {
      if (a.el === b.el) return 0;
      // DOCUMENT_POSITION_FOLLOWING (4): b comes after a.
      return a.el.compareDocumentPosition(b.el) & 4 ? -1 : 1;
    });
    return list.flatMap((e) => e.ids);
  }
}
