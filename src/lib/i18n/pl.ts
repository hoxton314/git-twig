import type { MessageKey } from "./en";
import type { Message } from "./types";
import { AREAS } from "./catalog";

/** Polish messages, merged from the area catalogs. */
export const pl: Record<MessageKey, Message> = {
  ...AREAS.shell.pl,
  ...AREAS.app.pl,
  ...AREAS.branches.pl,
  ...AREAS.config.pl,
  ...AREAS.diff.pl,
  ...AREAS.github.pl,
  ...AREAS.graph.pl,
  ...AREAS.operations.pl,
  ...AREAS.settings.pl,
  ...AREAS.staging.pl,
};
