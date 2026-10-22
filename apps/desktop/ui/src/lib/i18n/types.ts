// Types derived from the Indonesian source dictionary.

import type { id } from './messages/id';

/** The languages Kinora ships. */
export type Locale = 'id' | 'en';

/** A plural entry: one form per `Intl.PluralRules` category Kinora uses. */
export interface PluralForms {
  readonly one: string;
  readonly other: string;
}

/** The shape of `id` with every string widened, so other languages fit it. */
type Widen<T> = T extends string
  ? string
  : T extends PluralForms
    ? PluralForms
    : { readonly [K in keyof T]: Widen<T[K]> };

/** A complete dictionary. */
export type Messages = Widen<typeof id>;

/** Dotted paths to the entries of `T` whose value is `Leaf`. */
type PathsTo<T, Leaf> = {
  [K in keyof T & string]: T[K] extends Leaf
    ? K
    : T[K] extends string | PluralForms
      ? never
      : `${K}.${PathsTo<T[K], Leaf>}`;
}[keyof T & string];

/** Keys of plain messages, e.g. `'start.newProject'`. */
export type MessageKey = PathsTo<Messages, string>;

/** Keys of plural messages, e.g. `'units.clips'`. */
export type PluralKey = PathsTo<Messages, PluralForms>;

/** Values for `{placeholder}`s; a `MessageRef` is translated first. */
export type MessageParams = Readonly<Record<string, string | number | MessageRef>>;

/** A message to show later, e.g. an error mapped before the language is known. */
export interface MessageRef {
  readonly key: MessageKey;
  readonly params?: MessageParams;
}
