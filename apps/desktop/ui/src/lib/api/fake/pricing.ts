// Atrapa cennika modeli (`pricing_get`, `pricing_apply`): kilka pozycji z prawdziwego
// `providers-catalog/ceny/cennik.toml` i uproszczony odczyt bloków `[[price]]` (rdzeń używa parsera TOML
// z pełną kontrolą — tu tylko tyle, ile potrzeba testom interfejsu).
import type { PriceChangeRow, PriceRow, PricingView } from '../types';

type Entry = Omit<PriceRow, 'user'>;

const SOURCE_OPENAI = 'https://developers.openai.com/api/docs/pricing';
const SOURCE_ANTHROPIC = 'https://platform.claude.com/docs/en/about-claude/pricing';

const BUILTIN: readonly Entry[] = [
  entry('anthropic', 'claude-haiku-5-5', 0.1, 0.5, 0.01, 0.125, SOURCE_ANTHROPIC),
  entry('anthropic', 'claude-opus-5-5', 4, 20, 0.2, 5, SOURCE_ANTHROPIC),
  entry('openai', 'gpt-6-luna', 0.1, 0.5, 0.01, 0.125, SOURCE_OPENAI),
  entry('openai', 'gpt-6.1-sol', 2, 10, 0.1, 2.5, SOURCE_OPENAI, 'Powyżej 272K: 4.00 / 15.00.'),
  entry(
    'openrouter',
    'qwen/qwen3.8-flash',
    0.15,
    0.47,
    0.016,
    null,
    'https://openrouter.ai/api/v1/models',
  ),
];

function entry(
  provider: string,
  model: string,
  input: number,
  output: number,
  cache_read: number | null,
  cache_write: number | null,
  source: string,
  note: string | null = null,
): Entry {
  return { provider, model, input, output, cache_read, cache_write, source, note };
}

const key = (e: Entry) => `${e.provider}/${e.model}`;
const samePrice = (a: Entry, b: Entry) =>
  a.input === b.input &&
  a.output === b.output &&
  a.cache_read === b.cache_read &&
  a.cache_write === b.cache_write;

/** Odczyt bloków `[[price]]` (pola liczbowe i tekstowe); `null` — brak poprawnej pozycji. */
export function parsePriceUpdate(text: string): { checked: string; prices: Entry[] } | null {
  const checked = /^checked\s*=\s*"([^"]*)"/m.exec(text)?.[1] ?? '';
  const prices: Entry[] = [];
  for (const block of text.split('[[price]]').slice(1)) {
    const field = (name: string) => new RegExp(`^${name}\\s*=\\s*(.+)$`, 'm').exec(block)?.[1];
    const str = (name: string) => field(name)?.replace(/^"|"$/g, '') ?? '';
    const num = (name: string) => {
      const raw = field(name);
      const value = raw === undefined ? NaN : Number(raw);
      return Number.isFinite(value) && value >= 0 ? value : null;
    };
    const [input, output] = [num('input'), num('output')];
    if (!str('provider') || !str('model') || input === null || output === null) return null;
    prices.push(
      entry(
        str('provider'),
        str('model'),
        input,
        output,
        num('cache_read'),
        num('cache_write'),
        str('source'),
        str('note') || null,
      ),
    );
  }
  return prices.length > 0 ? { checked, prices } : null;
}

class FakePricing {
  private user = new Map<string, Entry>();
  private checked = '2026-10-20';

  private rows(): PriceRow[] {
    const all = new Map(BUILTIN.map((e) => [key(e), e]));
    for (const [k, e] of this.user) all.set(k, e);
    return [...all.values()]
      .sort((a, b) => key(a).localeCompare(key(b)))
      .map((e) => ({ ...e, user: this.user.has(key(e)) }));
  }

  view(changes: PriceChangeRow[], saved: boolean): PricingView {
    return {
      checked: this.checked,
      file: 'C:\\Users\\atrapa\\AppData\\Roaming\\Alfa\\config\\cennik.toml',
      rows: this.rows(),
      changes,
      saved,
    };
  }

  apply(update: { checked: string; prices: Entry[] }, save: boolean): PricingView {
    const now = new Map(this.rows().map((r) => [key(r), r]));
    const changes: PriceChangeRow[] = update.prices
      .filter((e) => {
        const before = now.get(key(e));
        return !before || !samePrice(before, e);
      })
      .map((e) => ({
        after: { ...e, user: true },
        before: now.get(key(e)) ?? null,
      }));
    if (save) {
      for (const e of update.prices) this.user.set(key(e), e);
      if (update.checked > this.checked) this.checked = update.checked;
    }
    return this.view(changes, save);
  }
}

export const fakePricing = new FakePricing();
