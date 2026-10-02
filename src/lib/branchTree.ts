/**
 * Folder grouping for the branch list: `feature/foo` and `feature/bar` become
 * a collapsible `feature` folder. Pure helpers, plus per-repo persistence of
 * which folders are collapsed.
 */
import type { BranchInfo } from "./types/git";

export interface BranchFolderNode {
  kind: "folder";
  /** Unique, stable key used for collapse state, e.g. "local:feature/ui". */
  key: string;
  label: string;
  children: BranchTreeNode[];
  /** Number of branches below this folder (recursive). */
  count: number;
}

export interface BranchLeafNode {
  kind: "branch";
  key: string;
  /** Last path segment of the branch name. */
  label: string;
  branch: BranchInfo;
}

export type BranchTreeNode = BranchFolderNode | BranchLeafNode;

/**
 * Build a folder tree. `nameOf` picks the path to split (e.g. the short name
 * for remote branches). Folders sort first (alphabetically); branches keep
 * the input order, so the backend's "HEAD first" ordering is preserved.
 */
export function buildBranchTree(
  list: BranchInfo[],
  nameOf: (b: BranchInfo) => string,
  keyPrefix: string,
): BranchTreeNode[] {
  const root: BranchFolderNode = { kind: "folder", key: keyPrefix, label: "", children: [], count: 0 };

  for (const branch of list) {
    const parts = nameOf(branch).split("/").filter((p) => p.length > 0);
    if (parts.length === 0) continue;
    let folder = root;
    folder.count++;
    let path = "";
    for (const segment of parts.slice(0, -1)) {
      path = path ? `${path}/${segment}` : segment;
      const key = `${keyPrefix}:${path}`;
      let next = folder.children.find(
        (c): c is BranchFolderNode => c.kind === "folder" && c.key === key,
      );
      if (!next) {
        next = { kind: "folder", key, label: segment, children: [], count: 0 };
        folder.children.push(next);
      }
      next.count++;
      folder = next;
    }
    folder.children.push({
      kind: "branch",
      key: `${keyPrefix}#${branch.name}`,
      label: parts[parts.length - 1],
      branch,
    });
  }

  sortFolders(root);
  return root.children;
}

function sortFolders(folder: BranchFolderNode) {
  const folders = folder.children.filter((c): c is BranchFolderNode => c.kind === "folder");
  const leaves = folder.children.filter((c) => c.kind === "branch");
  folders.sort((a, b) => a.label.localeCompare(b.label));
  folders.forEach(sortFolders);
  folder.children = [...folders, ...leaves];
}

/** Keys of every folder in the tree (for "expand/collapse all"). */
export function folderKeys(nodes: BranchTreeNode[]): string[] {
  const out: string[] = [];
  for (const n of nodes) {
    if (n.kind === "folder") {
      out.push(n.key);
      out.push(...folderKeys(n.children));
    }
  }
  return out;
}

// ── Persistence ──────────────────────────────────────────────────────

const STORAGE_PREFIX = "twig.branchList.collapsed:";

export function loadCollapsed(repoPath: string): Set<string> {
  try {
    const raw = localStorage.getItem(STORAGE_PREFIX + repoPath);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((k) => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

export function saveCollapsed(repoPath: string, collapsed: Set<string>) {
  try {
    if (collapsed.size === 0) localStorage.removeItem(STORAGE_PREFIX + repoPath);
    else localStorage.setItem(STORAGE_PREFIX + repoPath, JSON.stringify([...collapsed]));
  } catch {
    // Storage unavailable (private mode, quota): collapse state just won't persist.
  }
}

/** Copy text to the clipboard, falling back to a hidden textarea. */
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // Fall through: clipboard API unavailable or denied in this webview.
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.opacity = "0";
  document.body.appendChild(ta);
  ta.select();
  const ok = document.execCommand("copy");
  ta.remove();
  if (!ok) throw new Error("Clipboard is not available");
}
