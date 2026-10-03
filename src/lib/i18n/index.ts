/**
 * Translations. Catalogs are flat maps of keys to messages; English (`en.ts`)
 * is the source of truth and every other locale must have the same keys
 * (a test checks it).
 *
 *   import { t } from "../lib/i18n";
 *   $t("commit.button", { count: 3 })
 *
 * Messages interpolate `{name}` placeholders. A plural message is an object
 * of `Intl.PluralRules` categories (`one`, `few`, `many`, `other`, …) and
 * picks by the `count` parameter.
 */
import { derived, writable, fromStore } from "svelte/store";
import { en, type MessageKey } from "./en";
import { pl } from "./pl";
import type { Message } from "./types";
export type { Plural, Message } from "./types";
export type Catalog = Record<MessageKey, Message>;
export type { MessageKey };

export const CATALOGS = { en, pl } as const satisfies Record<string, Catalog>;
export type Locale = keyof typeof CATALOGS;
export const LOCALES: { id: Locale; name: string }[] = [
  { id: "en", name: "English" },
  { id: "pl", name: "Polski" },
];

/** The language setting: "system" or a locale id. */
export const languageSetting = writable<string>("system");

/** Pick a supported locale for a setting value and the system's languages. */
export function resolveLocale(setting: string, systemLanguages: readonly string[]): Locale {
  if (Object.hasOwn(CATALOGS, setting)) return setting as Locale;
  for (const tag of systemLanguages) {
    const base = tag.toLowerCase().split("-")[0];
    if (Object.hasOwn(CATALOGS, base)) return base as Locale;
  }
  return "en";
}

function systemLanguages(): readonly string[] {
  return typeof navigator !== "undefined" ? (navigator.languages ?? [navigator.language]) : [];
}

export const locale = derived(languageSetting, (s) => resolveLocale(s, systemLanguages()));

export type Params = Record<string, string | number>;

/** Format `key` in `loc` (falls back to English, then to the key). */
export function translate(loc: Locale, key: MessageKey, params: Params = {}): string {
  const msg = (CATALOGS[loc] as Catalog)[key] ?? (en as Catalog)[key];
  if (msg === undefined) return key;
  let text: string;
  if (typeof msg === "string") {
    text = msg;
  } else {
    const n = Number(params.count ?? 0);
    const rule = new Intl.PluralRules(loc).select(n);
    text = msg[rule] ?? msg.other;
  }
  return text.replace(/\{(\w+)\}/g, (m, name: string) => (name in params ? String(params[name]) : m));
}

/** Reactive translator: `$t("key", params)`. */
export const t = derived(locale, (loc) => (key: MessageKey, params?: Params) => translate(loc, key, params));

const currentLocale = fromStore(locale);

/**
 * Translation for plain TS modules and helpers. Reading the locale through
 * `fromStore` makes markup and `$derived` values that call it (directly or
 * via a helper) re-run when the language changes; elsewhere it is a plain read.
 */
export function tr(key: MessageKey, params?: Params): string {
  return translate(currentLocale.current, key, params);
}

/**
 * Split a translated message at its `{name}` placeholders, for markup that
 * wraps a value (`<strong>{branch}</strong>`): odd indices are placeholder
 * names, even indices plain text.
 */
export function splitMessage(msg: string): string[] {
  return msg.split(/\{(\w+)\}/);
}

/** Placeholder names used by a message (all plural forms). */
export function placeholders(msg: Message): string[] {
  const texts = typeof msg === "string" ? [msg] : Object.values(msg);
  return [...new Set(texts.flatMap((s) => [...(s ?? "").matchAll(/\{(\w+)\}/g)].map((m) => m[1])))].sort();
}
