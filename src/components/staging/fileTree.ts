/** Folder-tree model for the staging panel's file lists. */
import type { FileStatus } from "../../lib/types/git";

export interface TreeFile {
  kind: "file";
  /** Display name (last path segment; untracked dirs keep their "/"). */
  name: string;
  file: FileStatus;
}

export interface TreeDir {
  kind: "dir";
  /** Display name; single-child chains are compacted to "a/b/c". */
  name: string;
  /** Repo-relative folder path without trailing slash. */
  path: string;
  children: TreeNode[];
  /** Every file below this folder, for folder-level actions. */
  files: FileStatus[];
}

export type TreeNode = TreeFile | TreeDir;

export interface TreeRow {
  node: TreeNode;
  depth: number;
}

function sortNodes(nodes: TreeNode[]) {
  nodes.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "dir" ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  for (const n of nodes) if (n.kind === "dir") sortNodes(n.children);
}

/** Merge folders that only contain a single sub-folder ("src" > "lib" → "src/lib"). */
function compact(node: TreeDir): TreeDir {
  let cur = node;
  while (cur.children.length === 1 && cur.children[0].kind === "dir") {
    const only: TreeDir = cur.children[0];
    cur = { ...only, name: `${cur.name}/${only.name}` };
  }
  cur.children = cur.children.map((c) => (c.kind === "dir" ? compact(c) : c));
  return cur;
}

export function buildTree(files: FileStatus[]): TreeNode[] {
  const root: TreeDir = { kind: "dir", name: "", path: "", children: [], files: [] };
  for (const file of files) {
    // Untracked directories are reported as "dir/"; keep them as leaves.
    const isDirEntry = file.path.endsWith("/");
    const parts = file.path.replace(/\/+$/, "").split("/");
    const leaf = parts.pop() ?? file.path;
    let dir = root;
    dir.files.push(file);
    let prefix = "";
    for (const part of parts) {
      prefix = prefix ? `${prefix}/${part}` : part;
      let next = dir.children.find(
        (c): c is TreeDir => c.kind === "dir" && c.path === prefix,
      );
      if (!next) {
        next = { kind: "dir", name: part, path: prefix, children: [], files: [] };
        dir.children.push(next);
      }
      next.files.push(file);
      dir = next;
    }
    dir.children.push({ kind: "file", name: isDirEntry ? `${leaf}/` : leaf, file });
  }
  sortNodes(root.children);
  return root.children.map((c) => (c.kind === "dir" ? compact(c) : c));
}

/** Flatten the tree into visible rows, skipping collapsed folders' contents. */
export function visibleRows(
  nodes: TreeNode[],
  collapsed: ReadonlySet<string>,
  depth = 0,
  out: TreeRow[] = [],
): TreeRow[] {
  for (const node of nodes) {
    out.push({ node, depth });
    if (node.kind === "dir" && !collapsed.has(node.path)) {
      visibleRows(node.children, collapsed, depth + 1, out);
    }
  }
  return out;
}

/** Case-insensitive path substring filter. */
export function filterFiles(files: FileStatus[], query: string): FileStatus[] {
  const q = query.trim().toLowerCase();
  if (!q) return files;
  return files.filter((f) => f.path.toLowerCase().includes(q));
}

/** Parent folder of a repo-relative path, or "" at the root. */
export function parentDir(path: string): string {
  const clean = path.replace(/\/+$/, "");
  const i = clean.lastIndexOf("/");
  return i === -1 ? "" : clean.slice(0, i);
}

/** File extension (without the dot) of a path's last segment, if any. */
export function extension(path: string): string | null {
  const name = path.replace(/\/+$/, "").split("/").pop() ?? "";
  const i = name.lastIndexOf(".");
  return i > 0 && i < name.length - 1 ? name.slice(i + 1) : null;
}
