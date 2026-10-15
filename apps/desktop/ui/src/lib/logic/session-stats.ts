// Statystyki rozmowy (paleta `Ctrl+K` → „Statystyki rozmowy”): liczone w UI z bieżącej ścieżki
// i całego drzewa tur — wiadomości, słowa, czas czytania, tokeny, koszt, modele, narzędzia.
import type { Money, Turn } from '../api/types';

/** Tempo czytania (słowa na minutę) do szacunku czasu lektury. */
export const WORDS_PER_MINUTE = 200;

export interface ModelUse {
  readonly name: string;
  readonly replies: number;
}

export interface SessionStats {
  readonly userMessages: number;
  readonly agentMessages: number;
  readonly userWords: number;
  readonly agentWords: number;
  readonly characters: number;
  /** Minuty czytania całej ścieżki (zaokrąglone w górę; 0 dla pustej). */
  readonly readingMinutes: number;
  readonly inputTokens: number;
  readonly outputTokens: number;
  /** Suma kosztów odpowiedzi w walucie pierwszej odpowiedzi z kosztem (`null` — brak kosztów). */
  readonly cost: Money | null;
  readonly models: readonly ModelUse[];
  readonly tools: number;
  /** Średni czas odpowiedzi (ms) — tylko odpowiedzi z pomiarem. */
  readonly averageLatencyMs: number | null;
  readonly first: string | null;
  readonly last: string | null;
  /** Tury w drzewie poza bieżącą ścieżką (inne wersje po „edytuj” i „ponów”). */
  readonly otherBranches: number;
}

export function countWords(text: string): number {
  const words = text.trim().match(/[\p{L}\p{N}]+(?:['’-][\p{L}\p{N}]+)*/gu);
  return words ? words.length : 0;
}

export function sessionStats(path: readonly Turn[], treeSize: number): SessionStats {
  let userMessages = 0;
  let agentMessages = 0;
  let userWords = 0;
  let agentWords = 0;
  let characters = 0;
  let inputTokens = 0;
  let outputTokens = 0;
  let tools = 0;
  let cost = null as Money | null;
  let latencySum = 0;
  let latencyCount = 0;
  const models = new Map<string, number>();
  for (const turn of path) {
    const words = countWords(turn.text);
    characters += turn.text.length;
    tools += turn.tools.length;
    if (turn.author === 'user') {
      userMessages += 1;
      userWords += words;
    } else {
      agentMessages += 1;
      agentWords += words;
    }
    const usage = turn.usage;
    if (!usage) continue;
    inputTokens += usage.input_tokens;
    outputTokens += usage.output_tokens;
    if (usage.latency_ms > 0) {
      latencySum += usage.latency_ms;
      latencyCount += 1;
    }
    if (usage.model) {
      const name = usage.provider ? `${usage.provider} · ${usage.model}` : usage.model;
      models.set(name, (models.get(name) ?? 0) + 1);
    }
    if (usage.cost.minor > 0 && (!cost || cost.currency === usage.cost.currency)) {
      cost = { minor: (cost?.minor ?? 0) + usage.cost.minor, currency: usage.cost.currency };
    }
  }
  const totalWords = userWords + agentWords;
  return {
    userMessages,
    agentMessages,
    userWords,
    agentWords,
    characters,
    readingMinutes: totalWords ? Math.ceil(totalWords / WORDS_PER_MINUTE) : 0,
    inputTokens,
    outputTokens,
    cost,
    models: [...models]
      .map(([name, replies]) => ({ name, replies }))
      .sort((a, b) => b.replies - a.replies || a.name.localeCompare(b.name)),
    tools,
    averageLatencyMs: latencyCount ? Math.round(latencySum / latencyCount) : null,
    first: path[0]?.created_at ?? null,
    last: path.at(-1)?.created_at ?? null,
    otherBranches: Math.max(0, treeSize - path.length),
  };
}

/** Statystyki jako tekst (Markdown) do schowka. */
export function statsMarkdown(title: string, rows: readonly (readonly [string, string])[]): string {
  return [`## ${title}`, '', ...rows.map(([k, v]) => `- **${k}:** ${v}`)].join('\n');
}
