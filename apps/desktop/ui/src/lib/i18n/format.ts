// Dates and times for display, in the active language.

import type { Locale, MessageRef } from './types';

const DAY_MS = 86_400_000;

/** Midnight (local time) at the start of the day containing `ms`. */
function startOfDay(ms: number): number {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

/**
 * When something happened, for people: "Hari ini, 14.05", "Kemarin, 09.30",
 * then "12 Okt" this year or "12 Okt 2025" before. Returns a message for
 * today and yesterday, plain text otherwise.
 */
export function describeDate(ms: number, now: number, locale: Locale): MessageRef | string {
  const time = new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(ms);
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / DAY_MS);
  if (days === 0) return { key: 'time.today', params: { time } };
  if (days === 1) return { key: 'time.yesterday', params: { time } };
  const sameYear = new Date(ms).getFullYear() === new Date(now).getFullYear();
  return new Intl.DateTimeFormat(locale, {
    day: 'numeric',
    month: 'short',
    ...(sameYear ? {} : { year: 'numeric' }),
  }).format(ms);
}
