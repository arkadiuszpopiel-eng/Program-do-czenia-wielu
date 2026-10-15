// Atrapa: wbudowana instrukcja — trzy działy z HTML jak z rdzenia (tylko znaczniki z listy
// sanitizera) i wyszukiwanie po akapitach bez wielkości liter i polskich znaków.
import type { HelpApi } from '../client-help';
import type { HelpHit, HelpPage } from '../types-help';
import type { FakeCore } from './core';

const PAGES: readonly HelpPage[] = [
  {
    id: 'przewodnik',
    title: 'Alfa — przewodnik użytkownika',
    html_sanitized:
      '<h1>Alfa — przewodnik użytkownika</h1><p>Alfa to asystentka na Windows 11. Rozmawiasz z nią tekstem albo głosem.</p>',
  },
  {
    id: 'rozmowa',
    title: 'Rozmowa',
    html_sanitized:
      '<h1>Rozmowa</h1><h2>Polecenia bez modelu</h2><p>Na proste polecenia Alfa odpowiada sama — rachunki, literowanie, losowanie, data i godzina.</p><h2>Brak internetu</h2><p>Wiadomość do modelu w chmurze czeka w kolejce.</p>',
  },
  {
    id: 'bezpieczenstwo',
    title: 'Bezpieczeństwo',
    html_sanitized:
      '<h1>Bezpieczeństwo</h1><h2>Okno Brokera</h2><p>Każde ryzykowne działanie agentki wymaga Twojej zgody w oknie Brokera.</p>',
  },
];

const fold = (s: string): string =>
  s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l');

function search(query: string): HelpHit[] {
  const words = fold(query)
    .split(/\s+/)
    .filter((w) => w.length >= 2);
  if (!words.length) return [];
  const hits: HelpHit[] = [];
  for (const p of PAGES) {
    let section = '';
    for (const part of p.html_sanitized.split(/(?=<h2>)|(?=<p>)/)) {
      const text = part.replace(/<[^>]+>/g, ' ').trim();
      if (part.startsWith('<h2>')) section = text;
      const folded = fold(text);
      if (text && words.every((w) => folded.includes(w))) {
        hits.push({ id: p.id, title: p.title, section, snippet: text });
      }
    }
  }
  return hits;
}

export class FakeHelp {
  constructor(private readonly core: FakeCore) {}

  api(): HelpApi {
    return {
      topics: () => this.core.reply(PAGES.map(({ id, title }) => ({ id, title }))),
      page: async (id) => {
        const found = PAGES.find((p) => p.id === id);
        if (!found) throw new Error(`Nie znaleziono: dział instrukcji „${id}”`);
        return this.core.reply(found);
      },
      search: async (query) => this.core.reply(search(query)),
    };
  }
}
