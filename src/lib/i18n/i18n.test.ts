import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { CATALOGS, LOCALES, languageSetting, placeholders, resolveLocale, t, translate, type Catalog } from "./index";
import { en } from "./en";

const keys = Object.keys(en).sort();

describe("catalogs", () => {
  for (const [loc, catalog] of Object.entries(CATALOGS) as [string, Catalog][]) {
    it(`${loc} has exactly the English keys`, () => {
      expect(Object.keys(catalog).sort()).toEqual(keys);
    });

    it(`${loc} uses the same placeholders as English`, () => {
      for (const key of keys) {
        const k = key as keyof typeof en;
        expect(placeholders(catalog[k]), `${loc}: ${key}`).toEqual(placeholders(en[k]));
      }
    });

    it(`${loc} plural messages have an "other" form and valid categories`, () => {
      const valid = new Set(new Intl.PluralRules(loc).resolvedOptions().pluralCategories);
      for (const [key, msg] of Object.entries(catalog)) {
        if (typeof msg === "string") continue;
        expect(msg.other, key).toBeTypeOf("string");
        for (const cat of Object.keys(msg)) expect(valid.has(cat as Intl.LDMLPluralRule), `${key}: ${cat}`).toBe(true);
      }
    });
  }

  it("lists every catalog in LOCALES", () => {
    expect(LOCALES.map((l) => l.id).sort()).toEqual(Object.keys(CATALOGS).sort());
  });
});

describe("translate", () => {
  it("interpolates placeholders", () => {
    expect(translate("en", "commit.buttonCount", { count: 3 })).toBe("Commit (3)");
    expect(translate("en", "tabs.closeNamed", { name: "twig" })).toBe("Close twig");
  });

  it("leaves unknown placeholders as written", () => {
    expect(translate("en", "tabs.closeNamed")).toBe("Close {name}");
  });

  it("picks Polish plural forms", () => {
    expect(translate("pl", "status.otherOps", { count: 1 })).toBe("1 operacja w innych kartach");
    expect(translate("pl", "status.otherOps", { count: 3 })).toBe("3 operacje w innych kartach");
    expect(translate("pl", "status.otherOps", { count: 5 })).toBe("5 operacji w innych kartach");
    expect(translate("pl", "status.otherOps", { count: 22 })).toBe("22 operacje w innych kartach");
    expect(translate("pl", "time.daysAgo", { count: 1 })).toBe("1 dzień temu");
    expect(translate("pl", "time.daysAgo", { count: 2 })).toBe("2 dni temu");
  });
});

describe("resolveLocale", () => {
  it("uses an explicit supported setting", () => {
    expect(resolveLocale("pl", ["en-US"])).toBe("pl");
  });

  it("follows the system languages for 'system'", () => {
    expect(resolveLocale("system", ["pl-PL", "en-US"])).toBe("pl");
    expect(resolveLocale("system", ["de-DE", "en-GB"])).toBe("en");
  });

  it("falls back to English", () => {
    expect(resolveLocale("system", ["de-DE"])).toBe("en");
    expect(resolveLocale("system", [])).toBe("en");
    expect(resolveLocale("xx", [])).toBe("en");
  });
});

describe("t store", () => {
  it("follows the language setting", () => {
    languageSetting.set("pl");
    expect(get(t)("common.cancel")).toBe("Anuluj");
    languageSetting.set("en");
    expect(get(t)("common.cancel")).toBe("Cancel");
    languageSetting.set("system");
  });
});
