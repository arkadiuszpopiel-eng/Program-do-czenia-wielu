// Atrapa renderera Rust (pulldown-cmark + ammonia): prosty, bezpieczny HTML z escape'owanego
// tekstu. Obsługuje akapity, listy „- ", nagłówki „#" i bloki ``` — tyle, ile potrzeba makietom.
import type { RenderedBlock } from '../types';

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

interface RawBlock {
  readonly kind: 'text' | 'code';
  readonly lang: string | null;
  readonly body: string;
  readonly closed: boolean;
}

function splitBlocks(text: string): RawBlock[] {
  const blocks: RawBlock[] = [];
  const lines = text.split('\n');
  let para: string[] = [];
  const flushPara = (closed: boolean): void => {
    if (para.join('').trim())
      blocks.push({ kind: 'text', lang: null, body: para.join('\n'), closed });
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? '';
    const fence = /^```(\w*)\s*$/.exec(line);
    if (fence) {
      flushPara(true);
      const code: string[] = [];
      let j = i + 1;
      while (j < lines.length && !/^```\s*$/.test(lines[j] ?? '')) code.push(lines[j++] ?? '');
      const closed = j < lines.length;
      blocks.push({ kind: 'code', lang: fence[1] || null, body: code.join('\n'), closed });
      i = j;
      continue;
    }
    if (line.trim() === '') flushPara(true);
    else para.push(line);
  }
  flushPara(false);
  return blocks;
}

const LIST_ITEM = /^\s*[-*]\s+/;

function renderText(body: string): string {
  const lines = body.split('\n');
  const heading = /^(#{1,3})\s+(.*)$/.exec(body);
  if (heading && lines.length === 1) return `<h4>${escapeHtml(heading[2] ?? '')}</h4>`;
  // Kolejne linie „- " → lista; pozostałe → akapit (z <br> między liniami).
  const out: string[] = [];
  let para: string[] = [];
  let items: string[] = [];
  const flush = (): void => {
    if (para.length) out.push(`<p>${para.map(escapeHtml).join('<br>')}</p>`);
    if (items.length)
      out.push(`<ul>${items.map((i) => `<li>${escapeHtml(i)}</li>`).join('')}</ul>`);
    para = [];
    items = [];
  };
  for (const line of lines) {
    if (LIST_ITEM.test(line)) {
      if (para.length) flush();
      items.push(line.replace(LIST_ITEM, ''));
    } else {
      if (items.length) flush();
      para.push(line);
    }
  }
  flush();
  return out.join('');
}

/** Tekst odpowiedzi → bloki; `streaming` = ostatni blok pozostaje otwarty. */
export function renderBlocks(text: string, streaming: boolean): RenderedBlock[] {
  const raw = splitBlocks(text);
  return raw.map((block, index) => {
    const last = index === raw.length - 1;
    const closed = block.kind === 'code' ? block.closed : !(streaming && last);
    const lang = block.lang ? escapeHtml(block.lang) : null;
    const html_sanitized =
      block.kind === 'code'
        ? `<pre><code${lang ? ` class="language-${lang}"` : ''}>${escapeHtml(block.body)}</code></pre>`
        : renderText(block.body);
    return { index, kind: block.kind, lang: block.lang, html_sanitized, closed };
  });
}

/** Bloki zmienione względem poprzedniego renderu (tylko te idą w `TextDelta`). */
export function changedBlocks(
  previous: readonly RenderedBlock[],
  next: readonly RenderedBlock[],
): RenderedBlock[] {
  return next.filter((block) => {
    const old = previous[block.index];
    return !old || old.html_sanitized !== block.html_sanitized || old.closed !== block.closed;
  });
}

/** Podział na „tokeny" ~4 znaki (strumień ~100 tok/s w atrapie). */
export function tokenize(text: string): string[] {
  return text.match(/\s*\S{1,4}|\s+/g) ?? [];
}
