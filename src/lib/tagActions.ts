/** Tag actions shared by the tags sidebar and keybinding/palette actions. */
import { ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { TagInfo } from "./types/git";
import { refreshAll } from "./stores/graph";
import { refreshTags } from "./stores/tags";
import { toast, toastError } from "./stores/toasts";
import { confirmDestructive } from "./commitActions";
import { tr } from "./i18n";

async function refresh(path: string) {
  await Promise.all([refreshAll(path), refreshTags(path)]);
}

export async function pushTagAction(path: string, name?: string) {
  const failed = name ? tr("tags.pushTagFailed", { name }) : tr("tags.pushAllFailed");
  try {
    const r = await tauri.pushTag(path, name);
    if (r.success) {
      const dest = r.message.replace(/^Pushed to /, "");
      toast("success", name ? tr("tags.pushedTag", { name, dest }) : tr("tags.pushedAll", { dest }));
    } else toast("error", r.message.trim(), { title: failed });
  } catch (err) {
    toastError(failed, err);
  }
}

export async function deleteTagAction(path: string, tag: TagInfo) {
  const ok = await confirmDestructive(
    tr("tags.deleteConfirm", { name: tag.name }),
    tr("tags.deleteTitle"),
    tr("common.delete"),
  );
  if (!ok) return;
  try {
    const r = await tauri.deleteTag(path, tag.name);
    await refresh(path);
    if (!r.success) {
      toast("error", r.message.trim(), { title: tr("tags.deleteFailed") });
      return;
    }
    const target = tag.target_oid;
    toast("success", tr("tags.deleted", { name: tag.name }), {
      duration: 8000,
      action: target
        ? {
            label: tr("tags.undo"),
            run: async () => {
              try {
                const res = await tauri.createTag(path, tag.name, target, tag.message ?? undefined);
                await refresh(path);
                if (!res.success) toast("error", res.message.trim(), { title: tr("tags.undoFailed") });
              } catch (err) {
                toastError(tr("tags.undoFailed"), err);
              }
            },
          }
        : undefined,
    });
  } catch (err) {
    toastError(tr("tags.deleteFailed"), err);
  }
}

export async function deleteRemoteTagAction(path: string, tag: TagInfo) {
  // Deleting on the remote affects everyone, so always confirm.
  const ok = await ask(
    tr("tags.deleteRemoteConfirm", { name: tag.name }),
    {
      title: tr("tags.deleteRemoteTitle"),
      kind: "warning",
      okLabel: tr("common.delete"),
      cancelLabel: tr("common.cancel"),
    },
  );
  if (!ok) return;
  try {
    const r = await tauri.deleteRemoteTag(path, tag.name);
    if (r.success) toast("success", r.message.trim());
    else toast("error", r.message.trim(), { title: tr("tags.deleteRemoteFailed") });
  } catch (err) {
    toastError(tr("tags.deleteRemoteFailed"), err);
  }
}
