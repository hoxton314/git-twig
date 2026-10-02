/** Tag actions shared by the tags sidebar and keybinding/palette actions. */
import { ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { TagInfo } from "./types/git";
import { refreshAll } from "./stores/graph";
import { refreshTags } from "./stores/tags";
import { toast, toastError } from "./stores/toasts";
import { confirmDestructive } from "./commitActions";

async function refresh(path: string) {
  await Promise.all([refreshAll(path), refreshTags(path)]);
}

export async function pushTagAction(path: string, name?: string) {
  const what = name ? `tag ${name}` : "all tags";
  try {
    const r = await tauri.pushTag(path, name);
    if (r.success) toast("success", `Pushed ${what} (${r.message.replace(/^Pushed to /, "")})`);
    else toast("error", r.message.trim(), { title: `Push ${what} failed` });
  } catch (err) {
    toastError(`Push ${what} failed`, err);
  }
}

export async function deleteTagAction(path: string, tag: TagInfo) {
  const ok = await confirmDestructive(
    `Delete the local tag "${tag.name}"?\n\nThe tag on any remote is not affected.`,
    "Delete Tag",
    "Delete",
  );
  if (!ok) return;
  try {
    const r = await tauri.deleteTag(path, tag.name);
    await refresh(path);
    if (!r.success) {
      toast("error", r.message.trim(), { title: "Delete Tag Failed" });
      return;
    }
    const target = tag.target_oid;
    toast("success", `Deleted tag ${tag.name}`, {
      duration: 8000,
      action: target
        ? {
            label: "Undo",
            run: async () => {
              try {
                const res = await tauri.createTag(path, tag.name, target, tag.message ?? undefined);
                await refresh(path);
                if (!res.success) toast("error", res.message.trim(), { title: "Undo Failed" });
              } catch (err) {
                toastError("Undo Failed", err);
              }
            },
          }
        : undefined,
    });
  } catch (err) {
    toastError("Delete Tag Failed", err);
  }
}

export async function deleteRemoteTagAction(path: string, tag: TagInfo) {
  // Deleting on the remote affects everyone, so always confirm.
  const ok = await ask(
    `Delete the tag "${tag.name}" from the remote? Others who fetched it keep their copy, but it will no longer be on the server.`,
    { title: "Delete Remote Tag", kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" },
  );
  if (!ok) return;
  try {
    const r = await tauri.deleteRemoteTag(path, tag.name);
    if (r.success) toast("success", r.message.trim());
    else toast("error", r.message.trim(), { title: "Delete Remote Tag Failed" });
  } catch (err) {
    toastError("Delete Remote Tag Failed", err);
  }
}
