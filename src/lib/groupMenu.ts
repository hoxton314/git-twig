/** "Add to group" menu entries for a repository (tab bar, Home screen). */
import { get } from "svelte/store";
import type { MenuItem } from "../components/shared/ContextMenu.svelte";
import { repoGroups, addRepoToGroup, removeRepoFromGroup, createRepoGroup } from "./stores/repoHistory";
import { toast } from "./stores/toasts";

export function groupMenuItems(path: string, name: string): MenuItem[] {
  const groups = get(repoGroups);
  const items: MenuItem[] = groups.map((g) => {
    const member = g.paths.includes(path);
    return {
      label: member ? `✓ ${g.name}` : `Add to “${g.name}”`,
      action: () => {
        if (member) {
          removeRepoFromGroup(g.id, path);
          toast("info", `Removed ${name} from ${g.name}`);
        } else {
          addRepoToGroup(g.id, path, name);
          toast("success", `Added ${name} to ${g.name}`);
        }
      },
    };
  });
  if (items.length > 0) items.push({ separator: true });
  items.push({
    label: "New group with this repository",
    action: async () => {
      await createRepoGroup(name, [path]);
      toast("success", `Created a group for ${name}. Rename it on the Home screen.`);
    },
  });
  return items;
}
