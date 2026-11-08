// Formaty dat, liczb, walut i rozmiarów przez `Intl` (pl-PL / en-GB). Pieniądze w jednostkach
// drobnych — bez błędów zaokrągleń.
import type { Money } from '../api/types';

const tag = (locale: string): string => (locale === 'pl' ? 'pl-PL' : 'en-GB');
const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat | Intl.RelativeTimeFormat>();

function cached<T extends Intl.NumberFormat | Intl.DateTimeFormat | Intl.RelativeTimeFormat>(
  key: string,
  make: () => T,
): T {
  let value = cache.get(key) as T | undefined;
  if (!value) {
    value = make();
    cache.set(key, value);
  }
  return value;
}

export function formatMoney(locale: string, money: Money): string {
  const format = cached(
    `money:${locale}:${money.currency}`,
    () =>
      new Intl.NumberFormat(tag(locale), {
        style: 'currency',
        currency: money.currency,
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      }),
  );
  return format.format(money.minor / 100);
}

export function formatInteger(locale: string, n: number): string {
  return cached(`int:${locale}`, () => new Intl.NumberFormat(tag(locale))).format(Math.round(n));
}

export function formatPercent(locale: string, ratio: number): string {
  return cached(
    `pct:${locale}`,
    () => new Intl.NumberFormat(tag(locale), { style: 'percent', maximumFractionDigits: 0 }),
  ).format(ratio);
}

export function formatBytes(locale: string, bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  const number = cached(
    `bytes:${locale}:${digits}`,
    () => new Intl.NumberFormat(tag(locale), { maximumFractionDigits: digits }),
  ).format(value);
  return `${number} ${units[unit]}`;
}

/** Czas trwania: „820 ms", „2,1 s", „1:05". */
export function formatDuration(locale: string, ms: number): string {
  if (ms < 1000) return `${formatInteger(locale, ms)} ms`;
  if (ms < 60_000) {
    const s = cached(
      `dur:${locale}`,
      () => new Intl.NumberFormat(tag(locale), { maximumFractionDigits: 1 }),
    ).format(ms / 1000);
    return `${s} s`;
  }
  const total = Math.round(ms / 1000);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}

export function formatTime(locale: string, iso: string): string {
  return cached(
    `time:${locale}`,
    () => new Intl.DateTimeFormat(tag(locale), { hour: '2-digit', minute: '2-digit' }),
  ).format(new Date(iso));
}

export function formatDateTime(locale: string, iso: string): string {
  return cached(
    `dt:${locale}`,
    () => new Intl.DateTimeFormat(tag(locale), { dateStyle: 'full', timeStyle: 'short' }),
  ).format(new Date(iso));
}

export function formatDay(locale: string, iso: string): string {
  return cached(
    `day:${locale}`,
    () => new Intl.DateTimeFormat(tag(locale), { weekday: 'long', day: 'numeric', month: 'long' }),
  ).format(new Date(iso));
}

/** Klucz dnia w strefie lokalnej (separatory dni w rozmowie). */
export function dayKey(iso: string): string {
  const d = new Date(iso);
  return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
}

/** Względny znacznik czasu („2 min temu"); pełna data trafia do podpowiedzi. */
export function formatRelative(locale: string, iso: string, now: number = Date.now()): string {
  const diff = (new Date(iso).getTime() - now) / 1000;
  const abs = Math.abs(diff);
  const format = cached(
    `rel:${locale}`,
    () => new Intl.RelativeTimeFormat(tag(locale), { numeric: 'auto', style: 'short' }),
  );
  if (abs < 45) return format.format(0, 'second');
  if (abs < 3600) return format.format(Math.round(diff / 60), 'minute');
  if (abs < 86_400) return format.format(Math.round(diff / 3600), 'hour');
  if (abs < 7 * 86_400) return format.format(Math.round(diff / 86_400), 'day');
  return cached(
    `date:${locale}`,
    () => new Intl.DateTimeFormat(tag(locale), { day: 'numeric', month: 'short' }),
  ).format(new Date(iso));
}
