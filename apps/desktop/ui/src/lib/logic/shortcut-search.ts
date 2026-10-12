// Wyszukiwanie w ściągawce skrótów: po nazwie polecenia („wstecz”, „mikrofon”) albo po klawiszach
// („alt+1”, „ctrl k”). Bez rozróżniania wielkości liter i polskich znaków.

/** Małe litery bez znaków diakrytycznych (ł → l); odstępy i plusy jako jeden odstęp. */
export function foldText(text: string): string {
  return text
    .toLowerCase()
    .replace(/ł/g, 'l')
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .replace(/[\s+]+/g, ' ')
    .trim();
}

/** Czy skrót pasuje do zapytania: każde słowo zapytania musi wystąpić w nazwie albo w klawiszach. */
export function matchesShortcut(query: string, label: string, chords: readonly string[]): boolean {
  const words = foldText(query).split(' ').filter(Boolean);
  if (words.length === 0) return true;
  const hay = foldText([label, ...chords].join(' '));
  return words.every((word) => hay.includes(word));
}
