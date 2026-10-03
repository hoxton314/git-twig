/** A plural message: `Intl.PluralRules` categories, picked by the `count` param. */
export type Plural = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };
export type Message = string | Plural;

/** Shape of an English (source) area catalog. */
export type Source = Record<string, Message>;

/** A translation of a source catalog: the same keys. */
export type Translation<S extends Source> = Record<keyof S, Message>;
