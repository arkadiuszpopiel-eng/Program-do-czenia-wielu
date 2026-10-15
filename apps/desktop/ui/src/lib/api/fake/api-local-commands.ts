// Atrapa: Ustawienia → Polecenia bez modelu. Własne polecenia w pamięci (walidacja jak w rdzeniu:
// nazwa, frazy, odpowiedź, wyrażenia), tester z uproszczonym dopasowaniem i kilkoma
// wbudowanymi (rachunek dwóch liczb, godzina, literowanie, quiz). Pełna logika — `lib-quick`.
import type { LocalCommandsApi } from '../client-quick';
import type { BuiltinCommand, CustomCommand, QuickCommandsView, QuickTest } from '../types-quick';
import type { FakeCore } from './core';

const BUILTINS: readonly BuiltinCommand[] = [
  {
    title: 'Rachunki: działania, procenty, potęgi, pierwiastki, liczby słowami',
    examples: ['Ile to jest 7 razy 8?', '20% z 150'],
  },
  { title: 'Literowanie jednego słowa', examples: ['Przeliteruj rower'] },
  { title: 'Kostka, moneta, liczba losowa', examples: ['Rzuć kostką', 'orzeł czy reszka?'] },
  { title: 'Data i godzina z zegara komputera', examples: ['Która godzina?'] },
  {
    title: 'Quiz z tabliczki mnożenia (10 pytań, „koniec” przerywa)',
    examples: ['Przepytaj mnie z tabliczki mnożenia'],
  },
  {
    title: 'Rachunki na datach: dni do daty i od daty, dzień tygodnia, data za N dni',
    examples: ['Ile dni do 24 grudnia?', 'Jaki dzień tygodnia był 1 stycznia 2000?'],
  },
  {
    title: 'Przeliczanie jednostek: długość, masa, objętość, temperatura, prędkość, czas',
    examples: ['Ile to 5 km w milach?', 'ile cm ma cal'],
  },
  {
    title: 'Liczby słownie (także kwoty jak na fakturze) i liczby rzymskie',
    examples: ['1234 słownie', '2026 rzymskimi'],
  },
];

const fold = (s: string): string =>
  s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/ł/g, 'l')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim()
    .replace(/^(hej |czesc )?alfa /, '');

function matches(c: CustomCommand, message: string): string | null {
  const text = fold(message);
  for (const phrase of c.phrases) {
    const p = fold(phrase);
    if (c.mode === 'exact' && text === p) return c.reply;
    if (c.mode === 'contains' && ` ${text} `.includes(` ${p} `)) return c.reply;
    if (c.mode === 'starts' && (text === p || text.startsWith(`${p} `))) {
      const rest = message.trim().split(/\s+/).slice(p.split(' ').length).join(' ');
      return c.reply.replace('{reszta}', rest.replace(/^[^\p{L}\p{N}]+/u, ''));
    }
    if (c.mode === 'regex' && new RegExp(phrase, 'i').test(message)) return c.reply;
  }
  return null;
}

function builtin(message: string): QuickTest | null {
  const text = fold(message);
  const raw = message
    .toLowerCase()
    .trim()
    .replace(/[?!.]+$/, '');
  const math = /^(?:ile to (?:jest )?)?(\d+) ?(\+|-|razy|\*) ?(\d+)$/.exec(raw);
  if (math) {
    const [a, b] = [Number(math[1]), Number(math[3])];
    const op = math[2] ?? '+';
    const value = op === '+' ? a + b : op === '-' ? a - b : a * b;
    const shown = op === 'razy' || op === '*' ? '×' : op === '-' ? '−' : '+';
    return { source: 'builtin', name: 'rachunek', reply: `${a} ${shown} ${b} = **${value}**` };
  }
  if (/^(ktora|jaka) (jest )?godzina$/.test(text)) {
    return { source: 'builtin', name: 'data i godzina', reply: 'Jest godzina 14:30.' };
  }
  const spell = /^przeliteruj (\p{L}+)$/u.exec(text);
  if (spell?.[1]) {
    const letters = [...spell[1].toUpperCase()].join(' – ');
    return { source: 'builtin', name: 'literowanie', reply: `**${spell[1]}**: ${letters}.` };
  }
  if (/tabliczk\w* mnozenia/.test(text)) {
    return { source: 'quiz', name: null, reply: 'Dobrze, popytam cię z tabliczki mnożenia.' };
  }
  return null;
}

function check(c: CustomCommand): CustomCommand {
  const name = c.name.trim();
  const phrases = c.phrases.map((p) => p.trim()).filter(Boolean);
  const label = name || 'bez nazwy';
  if (!name || name.length > 60) throw new Error(`„${label}”: nazwa od 1 do 60 znaków`);
  if (!phrases.length || phrases.length > 10) throw new Error(`„${label}”: od 1 do 10 fraz`);
  for (const p of phrases) {
    if (c.mode === 'regex') {
      try {
        new RegExp(p);
      } catch {
        throw new Error(`„${label}”: złe wyrażenie „${p}”`);
      }
    } else if (!/[\p{L}\p{N}]/u.test(p)) {
      throw new Error(`„${label}”: fraza bez liter ani cyfr`);
    }
  }
  if (!c.reply.trim()) throw new Error(`„${label}”: odpowiedź od 1 do 2000 znaków`);
  return { ...c, name, phrases, reply: c.reply.trim() };
}

export class FakeLocalCommands {
  private commands: CustomCommand[] = [
    {
      id: 'dobranoc',
      name: 'Dobranoc',
      phrases: ['dobranoc', 'idę spać'],
      mode: 'exact',
      reply: 'Dobranoc! Do jutra.',
      enabled: true,
      priority: 0,
    },
  ];

  constructor(private readonly core: FakeCore) {}

  private view(): QuickCommandsView {
    return { commands: this.commands, builtins: BUILTINS, enabled: true };
  }

  private set(list: readonly CustomCommand[]): QuickCommandsView {
    if (list.length > 50) throw new Error('najwyżej 50 poleceń');
    const out: CustomCommand[] = [];
    for (const c of list) {
      const ok = check(c);
      const taken = new Set(out.map((o) => o.id));
      let id = ok.id || fold(ok.name).replace(/ /g, '-') || 'polecenie';
      for (let n = 2; taken.has(id); n += 1) id = `${fold(ok.name).replace(/ /g, '-')}-${n}`;
      out.push({ ...ok, id });
    }
    this.commands = out;
    return this.view();
  }

  private test(message: string): QuickTest {
    for (const c of [...this.commands].sort((a, b) => b.priority - a.priority)) {
      const reply = c.enabled ? matches(c, message) : null;
      if (reply !== null) return { source: 'custom', name: c.name, reply };
    }
    return builtin(message) ?? { source: 'model', name: null, reply: null };
  }

  api(): LocalCommandsApi {
    return {
      get: () => this.core.reply(this.view()),
      set: async (commands) => this.core.reply(this.set(commands)),
      test: async (message) => this.core.reply(this.test(message)),
    };
  }
}
