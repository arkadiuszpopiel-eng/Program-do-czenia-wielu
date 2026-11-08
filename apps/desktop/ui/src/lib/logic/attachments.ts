// Logika załączników composera bez DOM: suma tokenów, próg „dużych załączników”, szacunek kosztu
// ze stawki ostatnich odpowiedzi w sesji (koszt / tokeny wejścia), wskazówki z `DataTransfer`.
import type { ModelProfile, Turn } from '../api/types';
import type { AttachmentInfo, DroppedFileHint } from '../api/types-files';

/** Od tylu tokenów wejścia composer pokazuje szacunek (PLAN §14.8, docs/UI.md §8). */
export const LARGE_ATTACHMENT_TOKENS = 8_000;

export function totalTokens(list: readonly AttachmentInfo[]): number {
  return list.reduce((sum, a) => sum + a.tokens, 0);
}

export function totalBytes(list: readonly AttachmentInfo[]): number {
  return list.reduce((sum, a) => sum + a.bytes, 0);
}

/** Szacunek kosztu wejścia w groszach albo `null`, gdy nie ma z czego go policzyć. */
export interface CostEstimate {
  /** Koszt w groszach (zaokrąglony w górę) albo 0 dla profilu lokalnego. */
  readonly minor: number;
  /** `local` — model lokalny (bez kosztu), `rate` — stawka z ostatnich odpowiedzi. */
  readonly basis: 'local' | 'rate';
}

/**
 * Stawka za token wejścia z ostatnich (≤ 5) odpowiedzi z kosztem w sesji: koszt / (wejście + wyjście)
 * — zawyża wejście (wyjście bywa droższe), więc szacunek jest ostrożny (górny).
 */
export function estimateCost(
  tokens: number,
  profile: ModelProfile,
  turns: readonly Turn[],
): CostEstimate | null {
  if (tokens <= 0) return null;
  if (profile === 'local') return { minor: 0, basis: 'local' };
  const priced = turns
    .filter((t) => t.usage && t.usage.cost.minor > 0)
    .slice(-5)
    .map((t) => t.usage)
    .filter((u): u is NonNullable<Turn['usage']> => u !== null);
  const usedTokens = priced.reduce((sum, u) => sum + u.input_tokens + u.output_tokens, 0);
  const cost = priced.reduce((sum, u) => sum + u.cost.minor, 0);
  if (usedTokens <= 0 || cost <= 0) return null;
  return { minor: Math.max(1, Math.ceil((cost / usedTokens) * tokens)), basis: 'rate' };
}

/** Udział w oknie kontekstu (0–1). */
export function contextShare(tokens: number, maxTokens: number): number {
  return maxTokens > 0 ? Math.min(1, tokens / maxTokens) : 0;
}

/** Wskazówki z plików przeciągniętych w przeglądarce (atrapa; w Tauri ścieżki zna powłoka). */
export function hintsFrom(
  files: ArrayLike<{ name: string; size: number; type: string }>,
): DroppedFileHint[] {
  return Array.from(files, (f) => ({
    name: f.name,
    bytes: f.size,
    mime: f.type || 'application/octet-stream',
  }));
}

/** Czy przeciągane są pliki (a nie np. zaznaczony tekst). */
export function carriesFiles(types: readonly string[] | null | undefined): boolean {
  return Boolean(types?.includes('Files'));
}
