/** "Add to group" menu entries for a repository (tab bar, Home screen). */
import { get } from "svelte/store";
import type { MenuItem } from "../components/shared/ContextMenu.svelte";
import { repoGroups, addRepoToGroup, removeRepoFromGroup, createRepoGroup } from "./stores/repoHistory";
import { toast } from "./stores/toasts";
import { tr } from "./i18n";

export function groupMenuItems(path: string, name: string): MenuItem[] {
  const groups = get(repoGroups);
  const items: MenuItem[] = groups.map((g) => {
    const member = g.paths.includes(path);
    return {
      label: member ? `✓ ${g.name}` : tr("groups.addTo", { group: g.name }),
      action: () => {
        if (member) {
          removeRepoFromGroup(g.id, path);
          toast("info", tr("groups.removedFrom", { name, group: g.name }));
        } else {
          addRepoToGroup(g.id, path, name);
          toast("success", tr("groups.addedTo", { name, group: g.name }));
        }
      },
    };
  });
  if (items.length > 0) items.push({ separator: true });
  items.push({
    label: tr("groups.newWithRepo"),
    action: async () => {
      await createRepoGroup(name, [path]);
      toast("success", tr("groups.createdFor", { name }));
    },
  });
  return items;
}
