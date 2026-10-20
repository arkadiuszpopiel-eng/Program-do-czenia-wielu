// Kanwa artefaktów (C2.8, ADR 0009): wykrywanie artefaktów w odpowiedziach agentek.
// Logika bez DOM — źródło bloku kodu odtwarzane z HTML wyrenderowanego w Rust (tekst escapowany),
// bez parsera HTML w oknie głównym (Trusted Types).
import type { RenderedBlock } from '../api/types';
import type { CanvasKind } from '../api/types-files';

/** Limit źródła artefaktu (bajty UTF-8) — ten sam co w rdzeniu (`ARTIFACT_MAX_BYTES`). */
export const CANVAS_MAX_BYTES = 512 * 1024;
/** Odpowiedź uznana za dokument: co najmniej tyle znaków… */
export const DOC_MIN_CHARS = 800;
/** …i tyle nagłówków Markdown (`#`, `##`, `###`). */
export const DOC_MIN_HEADINGS = 2;

/** Skąd pochodzi artefakt — od tego zależy „Zapisz jako plik” (istniejące ścieżki zapisu). */
export type CanvasOrigin =
  | { readonly type: 'block'; readonly turnId: string; readonly blockIndex: number }
  | { readonly type: 'turn'; readonly sessionId: string; readonly turnId: string }
  | { readonly type: 'file'; readonly artifactId: string };

export interface CanvasItem {
  /** Stały klucz (np. do `aria-controls` i porównania „ten sam artefakt”). */
  readonly key: string;
  readonly kind: CanvasKind;
  /** Tytuł z treści (`<title>`, `<h1>`, nagłówek Markdown, nazwa pliku); `null` — wg rodzaju. */
  readonly title: string | null;
  readonly source: string;
  readonly origin: CanvasOrigin;
  /** Markdown już wyrenderowany w Rust (bloki tury) — bez dodatkowego wywołania rdzenia. */
  readonly html_sanitized?: string;
}

const LANGS: Readonly<Record<string, CanvasKind>> = {
  html: 'html',
  htm: 'html',
  xhtml: 'html',
  svg: 'svg',
  markdown: 'markdown',
  md: 'markdown',
};

/** Rodzaj artefaktu z języka bloku kodu (`xml` tylko, gdy treść jest rysunkiem SVG). */
export function kindOfLang(lang: string | null, source: string): CanvasKind | null {
  const key = (lang ?? '').trim().toLowerCase();
  if (key === 'xml') return /<svg[\s>]/i.test(source) ? 'svg' : null;
  return LANGS[key] ?? null;
}

/** Rodzaj artefaktu z typu MIME albo rozszerzenia pliku (panel Pliki). */
export function kindOfFile(name: string, mime: string): CanvasKind | null {
  const m = mime.toLowerCase();
  const ext = /\.([a-z0-9]+)$/i.exec(name)?.[1]?.toLowerCase() ?? '';
  if (m === 'image/svg+xml' || ext === 'svg') return 'svg';
  if (m === 'text/html' || ext === 'html' || ext === 'htm') return 'html';
  if (m === 'text/markdown' || ext === 'md' || ext === 'markdown') return 'markdown';
  return null;
}

const ENTITIES: Readonly<Record<string, string>> = {
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  amp: '&',
};

/** Odwraca escapowanie tekstu z HTML rdzenia (encje nazwane i liczbowe, jedno przejście). */
export function decodeEntities(text: string): string {
  return text.replace(/&(#x[0-9a-f]+|#[0-9]+|[a-z]+);/gi, (whole, body: string) => {
    if (body[0] === '#') {
      const hex = body[1] === 'x' || body[1] === 'X';
      const code = Number.parseInt(body.slice(hex ? 2 : 1), hex ? 16 : 10);
      return Number.isFinite(code) && code > 0 && code <= 0x10ffff
        ? String.fromCodePoint(code)
        : whole;
    }
    return ENTITIES[body.toLowerCase()] ?? whole;
  });
}

/** Surowe źródło bloku kodu: treść `<code>` z HTML rdzenia (bez znaczników, encje odwrócone). */
export function codeSource(html: string): string {
  const open = /<code\b[^>]*>/i.exec(html);
  const start = open ? open.index + open[0].length : 0;
  const end = html.lastIndexOf('</code>');
  const inner = html.slice(start, end >= start ? end : html.length);
  return decodeEntities(inner.replace(/<[^>]*>/g, ''));
}

/** Rozmiar źródła w bajtach UTF-8. */
export function byteSize(text: string): number {
  return new TextEncoder().encode(text).length;
}

export const tooLarge = (item: Pick<CanvasItem, 'source'>): boolean =>
  byteSize(item.source) > CANVAS_MAX_BYTES;

const clip = (text: string): string | null => {
  const flat = text.replace(/\s+/g, ' ').trim();
  if (!flat) return null;
  return flat.length > 80 ? `${flat.slice(0, 79)}…` : flat;
};

/** Tytuł artefaktu z jego treści albo `null`. */
export function titleOf(kind: CanvasKind, source: string): string | null {
  if (kind === 'markdown') return clip(/^#{1,3}\s+(.+)$/m.exec(source)?.[1] ?? '');
  const tag = (name: string) =>
    new RegExp(`<${name}\\b[^>]*>([\\s\\S]*?)</${name}>`, 'i').exec(source)?.[1] ?? '';
  const text = (html: string) => decodeEntities(html.replace(/<[^>]*>/g, ''));
  return clip(text(tag('title'))) ?? clip(text(tag('h1')));
}

/** Czy tekst odpowiedzi jest dokumentem Markdown (długi, z nagłówkami). */
export function isDocument(text: string): boolean {
  if (text.length < DOC_MIN_CHARS) return false;
  const headings = text.match(/^#{1,3}\s+\S/gm)?.length ?? 0;
  return headings >= DOC_MIN_HEADINGS;
}

interface TurnLike {
  readonly id: string;
  readonly session_id: string;
  readonly author: string;
  readonly status: string;
  readonly text: string;
  readonly blocks: readonly RenderedBlock[];
}

/** Artefakt z zamkniętego bloku kodu (`html`, `svg`, `markdown`) albo `null`. */
export function blockArtifact(turnId: string, block: RenderedBlock): CanvasItem | null {
  if (block.kind !== 'code' || !block.closed) return null;
  const source = codeSource(block.html_sanitized);
  const kind = kindOfLang(block.lang, source);
  if (!kind || !source.trim()) return null;
  return {
    key: `${turnId}:${block.index}`,
    kind,
    title: titleOf(kind, source),
    source,
    origin: { type: 'block', turnId, blockIndex: block.index },
  };
}

/** Cała odpowiedź jako dokument Markdown (HTML z bloków tury — już z Rust) albo `null`. */
export function documentArtifact(turn: TurnLike): CanvasItem | null {
  if (turn.author === 'user' || turn.status !== 'complete' || !isDocument(turn.text)) return null;
  return {
    key: `${turn.id}:doc`,
    kind: 'markdown',
    title: titleOf('markdown', turn.text),
    source: turn.text,
    origin: { type: 'turn', sessionId: turn.session_id, turnId: turn.id },
    html_sanitized: turn.blocks.map((b) => b.html_sanitized).join('\n'),
  };
}

/** Wszystkie artefakty odpowiedzi: dokument (jeśli jest) i bloki kodu w kolejności. */
export function detectArtifacts(turn: TurnLike): CanvasItem[] {
  if (turn.author === 'user' || turn.status !== 'complete') return [];
  const doc = documentArtifact(turn);
  const blocks = turn.blocks
    .map((b) => blockArtifact(turn.id, b))
    .filter((a): a is CanvasItem => a !== null);
  return doc ? [doc, ...blocks] : blocks;
}
