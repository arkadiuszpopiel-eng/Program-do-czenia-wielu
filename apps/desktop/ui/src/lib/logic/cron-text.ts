// Opis wyrażenia cron (5 pól) słowami dla typowych harmonogramów: „codziennie o 08:00”,
// „w dni robocze o 08:00”, „co 15 min”. Wyrażenie, którego nie umiemy opisać pewnie, zwraca `null`
// — widok pokazuje je wtedy dosłownie (lepsze surowe niż błędny opis).

type Locale = 'pl' | 'en';

const DAYS: Record<Locale, readonly string[]> = {
  pl: ['niedz.', 'pon.', 'wt.', 'śr.', 'czw.', 'pt.', 'sob.'],
  en: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
};

const TEXT = {
  pl: {
    daily: (at: string) => `codziennie o ${at}`,
    weekdays: (at: string) => `w dni robocze o ${at}`,
    weekend: (at: string) => `w weekendy o ${at}`,
    days: (days: string, at: string) => `${days} o ${at}`,
    monthly: (day: number, at: string) => `${day}. dnia miesiąca o ${at}`,
    everyMinutes: (n: number) => (n === 1 ? 'co minutę' : `co ${n} min`),
    everyHours: (n: number, minute: string) =>
      n === 1 ? `co godzinę, minuta ${minute}` : `co ${n} godz., minuta ${minute}`,
  },
  en: {
    daily: (at: string) => `every day at ${at}`,
    weekdays: (at: string) => `on weekdays at ${at}`,
    weekend: (at: string) => `on weekends at ${at}`,
    days: (days: string, at: string) => `${days} at ${at}`,
    monthly: (day: number, at: string) => `on day ${day} of the month at ${at}`,
    everyMinutes: (n: number) => (n === 1 ? 'every minute' : `every ${n} min`),
    everyHours: (n: number, minute: string) =>
      n === 1 ? `every hour at minute ${minute}` : `every ${n} h at minute ${minute}`,
  },
} as const;

const int = (text: string, min: number, max: number): number | null => {
  if (!/^\d{1,2}$/.test(text)) return null;
  const n = Number(text);
  return n >= min && n <= max ? n : null;
};

const pad = (n: number): string => String(n).padStart(2, '0');

/** Dni tygodnia z pola cron (`1-5`, `1,3,5`, `0`, `7`) jako posortowana lista 0–6 (0 = niedziela). */
function weekdays(spec: string): number[] | null {
  const out = new Set<number>();
  for (const part of spec.split(',')) {
    const [a = '', b] = part.split('-');
    const lo = int(a, 0, 7);
    const hi = b === undefined ? lo : int(b, 0, 7);
    if (lo === null || hi === null || lo > hi) return null;
    for (let d = lo; d <= hi; d++) out.add(d % 7);
  }
  return [...out].sort((x, y) => x - y);
}

export function describeCron(expr: string, locale: Locale = 'pl'): string | null {
  const fields = expr.trim().split(/\s+/);
  if (fields.length !== 5) return null;
  const [minute = '', hour = '', day = '', month = '', dow = ''] = fields;
  const text = TEXT[locale];
  if (month !== '*') return null;

  // Co n minut / co n godzin.
  if (hour === '*' && day === '*' && dow === '*') {
    if (minute === '*') return text.everyMinutes(1);
    const step = /^\*\/(\d{1,2})$/.exec(minute);
    const n = step ? int(step[1] ?? '', 1, 59) : null;
    if (n !== null) return text.everyMinutes(n);
    const m = int(minute, 0, 59);
    return m === null ? null : text.everyHours(1, pad(m));
  }
  const m = int(minute, 0, 59);
  if (m === null) return null;
  const hourStep = /^\*\/(\d{1,2})$/.exec(hour);
  if (hourStep && day === '*' && dow === '*') {
    const n = int(hourStep[1] ?? '', 1, 23);
    return n === null ? null : text.everyHours(n, pad(m));
  }
  const h = int(hour, 0, 23);
  if (h === null) return null;
  const at = `${pad(h)}:${pad(m)}`;

  if (day !== '*') {
    const d = int(day, 1, 31);
    return d !== null && dow === '*' ? text.monthly(d, at) : null;
  }
  if (dow === '*') return text.daily(at);
  const days = weekdays(dow);
  if (!days || days.length === 0) return null;
  const key = days.join(',');
  if (key === '0,1,2,3,4,5,6') return text.daily(at);
  if (key === '1,2,3,4,5') return text.weekdays(at);
  if (key === '0,6') return text.weekend(at);
  // Tydzień od poniedziałku: niedziela (0) na końcu listy.
  const ordered = [...days.filter((d) => d !== 0), ...days.filter((d) => d === 0)];
  return text.days(ordered.map((d) => DAYS[locale][d]).join(', '), at);
}
