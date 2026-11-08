// „Kopiuj rozmowę (Markdown)” (paleta `Ctrl+K`): bieżąca ścieżka rozmowy do schowka w układzie
// eksportu z rdzenia (tytuł, autor z oznaczeniem „AI” i modelem, czas) z notą o treści wygenerowanej
// przez AI (AI Act, art. 50) — bez nagłówka YAML i załączników, bo to kopia, nie plik.
import type { Turn } from '../api/types';

export interface CopyLabels {
  readonly you: string;
  readonly agent: (id: string) => string;
  readonly when: (iso: string) => string;
  readonly aiNote: string;
}

export function conversationMarkdown(
  title: string,
  path: readonly Turn[],
  labels: CopyLabels,
): string {
  const turns = path.filter((t) => t.text.trim());
  const ai = turns.some((t) => t.author !== 'user');
  const parts = [`# ${title.trim()}`];
  if (ai) parts.push(`> ${labels.aiNote}`);
  for (const turn of turns) {
    const who =
      turn.author === 'user'
        ? labels.you
        : `${labels.agent(turn.author)} · AI${turn.usage?.model ? `, ${turn.usage.model}` : ''}`;
    parts.push('---', `### ${who} · ${labels.when(turn.created_at)}`, turn.text.trim());
  }
  return `${parts.join('\n\n')}\n`;
}
