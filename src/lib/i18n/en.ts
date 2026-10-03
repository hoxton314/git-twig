/**
 * English messages: the source catalog, merged from the area catalogs in
 * `catalog/`. Keys are `area.name`; every other locale has exactly these
 * keys. Plurals are objects keyed by Intl.PluralRules categories.
 */
import { AREAS } from "./catalog";

export const en = {
  ...AREAS.shell.en,
  ...AREAS.app.en,
  ...AREAS.branches.en,
  ...AREAS.config.en,
  ...AREAS.diff.en,
  ...AREAS.github.en,
  ...AREAS.graph.en,
  ...AREAS.operations.en,
  ...AREAS.settings.en,
  ...AREAS.staging.en,
};

export type MessageKey = keyof typeof en;
