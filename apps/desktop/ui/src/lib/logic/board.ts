// Tablica (przypięte odpowiedzi): wyszukiwanie bez wielkości liter i polskich znaków (każde słowo
// zapytania w treści, tytule rozmowy albo imieniu agentki) i eksport wszystkiego jako Markdown.
import type { BoardItem } from '../api/types-board';
import { foldText } from './shortcut-search';

export function filterBoard(
  items: readonly BoardItem[],
  query: string,
  agentName: (id: string) => string,
): readonly BoardItem[] {
  const words = foldText(query).split(/\s+/).filter(Boolean);
  if (!words.length) return items;
  return items.filter((item) => {
    const hay = foldText(`${item.text} ${item.session_title} ${agentName(item.agent)}`);
    return words.every((w) => hay.includes(w));
  });
}

/** Wszystkie pozycje jako Markdown: nagłówek z agentką, rozmową i czasem, potem treść. */
export function boardMarkdown(
  title: string,
  items: readonly BoardItem[],
  agentName: (id: string) => string,
  when: (iso: string) => string,
): string {
  const parts = items.map(
    (item) =>
      `## ${agentName(item.agent)} · ${item.session_title} · ${when(item.pinned_at)}\n\n${item.text.trim()}`,
  );
  return [`# ${title}`, ...parts].join('\n\n');
}
