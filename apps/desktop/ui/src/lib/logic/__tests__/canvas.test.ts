import { describe, expect, it } from 'vitest';
import type { RenderedBlock } from '../../api/types';
import {
  CANVAS_CHAT_MIN,
  CANVAS_MAX_BYTES,
  blockArtifact,
  byteSize,
  canvasDock,
  clipBytes,
  codeSource,
  decodeEntities,
  detectArtifacts,
  isDocument,
  kindOfFile,
  kindOfLang,
  titleOf,
  tooLarge,
} from '../canvas';

// HTML bloku kodu tak, jak go zwraca rdzeń (pulldown-cmark + ammonia): treść escapowana.
const code = (index: number, lang: string | null, html: string, closed = true): RenderedBlock => ({
  index,
  kind: 'code',
  lang,
  html_sanitized: `<pre data-code="0" data-lines="1"><code${lang ? ` data-lang="${lang}"` : ''}>${html}</code></pre>\n`,
  closed,
});
const text = (index: number, html: string): RenderedBlock => ({
  index,
  kind: 'text',
  lang: null,
  html_sanitized: html,
  closed: true,
});

const turn = (
  blocks: RenderedBlock[],
  extra: Partial<{ text: string; author: string; status: string }> = {},
) => ({
  id: 's1:t2',
  session_id: 's1',
  author: 'delta',
  status: 'complete',
  text: 'krótko',
  blocks,
  ...extra,
});

const longDoc = `# Raport kwartalny\n\n${'Akapit z treścią raportu. '.repeat(30)}\n\n## Wnioski\n\n${'Kolejne zdanie wniosków. '.repeat(10)}`;

describe('kanwa: rodzaje', () => {
  it('język bloku → rodzaj; xml tylko z rysunkiem SVG', () => {
    expect(kindOfLang('HTML', '<p>')).toBe('html');
    expect(kindOfLang('htm', '')).toBe('html');
    expect(kindOfLang('svg', '')).toBe('svg');
    expect(kindOfLang('md', '')).toBe('markdown');
    expect(kindOfLang('markdown', '')).toBe('markdown');
    expect(kindOfLang('xml', '<svg viewBox="0 0 1 1"/>')).toBe('svg');
    expect(kindOfLang('xml', '<note/>')).toBeNull();
    expect(kindOfLang('ts', '<p>')).toBeNull();
    expect(kindOfLang(null, '<p>')).toBeNull();
  });

  it('plik → rodzaj (MIME albo rozszerzenie)', () => {
    expect(kindOfFile('rys.SVG', 'application/octet-stream')).toBe('svg');
    expect(kindOfFile('a', 'image/svg+xml')).toBe('svg');
    expect(kindOfFile('strona.html', 'text/plain')).toBe('html');
    expect(kindOfFile('notatka.md', 'text/plain')).toBe('markdown');
    expect(kindOfFile('raport.pdf', 'application/pdf')).toBeNull();
  });
});

describe('kanwa: źródło z HTML rdzenia', () => {
  it('encje w jednym przejściu (bez podwójnego dekodowania)', () => {
    expect(decodeEntities('&lt;p class=&quot;a&quot;&gt;&amp;lt;&#39;&#x41;&nbsp;')).toBe(
      '<p class="a">&lt;\'A ',
    );
    expect(decodeEntities('&nieznana; &#0; &#x110000;')).toBe('&nieznana; &#0; &#x110000;');
  });

  it('treść <code> bez znaczników podświetlania', () => {
    const html = code(0, 'html', '&lt;h1&gt;Hej&lt;/h1&gt;\n<span class="tok-kw">x</span>\n');
    expect(codeSource(html.html_sanitized)).toBe('<h1>Hej</h1>\nx\n');
  });

  it('rozmiar w bajtach UTF-8 i limit', () => {
    expect(byteSize('ąę')).toBe(4);
    expect(tooLarge({ source: 'a'.repeat(CANVAS_MAX_BYTES) })).toBe(false);
    expect(tooLarge({ source: 'a'.repeat(CANVAS_MAX_BYTES + 1) })).toBe(true);
  });
});

describe('kanwa: tytuł', () => {
  it('HTML: <title>, potem <h1>; Markdown: pierwszy nagłówek', () => {
    expect(titleOf('html', '<title> Mój &amp; raport </title><h1>X</h1>')).toBe('Mój & raport');
    expect(titleOf('html', '<h1 class="t">Witaj <b>świecie</b></h1>')).toBe('Witaj świecie');
    expect(titleOf('svg', '<svg><text>a</text></svg>')).toBeNull();
    expect(titleOf('markdown', 'wstęp\n## Plan na tydzień\n')).toBe('Plan na tydzień');
    expect(titleOf('markdown', '# ' + 'x'.repeat(200))?.length).toBe(80);
  });
});

describe('kanwa: wykrywanie w odpowiedzi', () => {
  it('bloki html / svg / markdown — zamknięte, niepuste', () => {
    const t = turn([
      text(0, '<p>Oto strona:</p>'),
      code(1, 'html', '&lt;title&gt;Strona&lt;/title&gt;&lt;p&gt;a&lt;/p&gt;\n'),
      code(2, 'svg', '&lt;svg viewBox="0 0 10 10"&gt;&lt;/svg&gt;\n'),
      code(3, 'ts', 'const a = 1;\n'),
      code(4, 'md', '# Notatka\n'),
      code(5, 'html', '   \n'),
      code(6, 'html', '&lt;p&gt;', false),
    ]);
    const found = detectArtifacts(t);
    expect(found.map((a) => [a.kind, a.origin])).toEqual([
      ['html', { type: 'block', turnId: 's1:t2', blockIndex: 1 }],
      ['svg', { type: 'block', turnId: 's1:t2', blockIndex: 2 }],
      ['markdown', { type: 'block', turnId: 's1:t2', blockIndex: 4 }],
    ]);
    expect(found[0]?.title).toBe('Strona');
    expect(found[0]?.source).toBe('<title>Strona</title><p>a</p>\n');
    expect(found[0]?.key).toBe('s1:t2:1');
  });

  it('długi dokument Markdown z nagłówkami → artefakt całej odpowiedzi', () => {
    expect(isDocument(longDoc)).toBe(true);
    expect(isDocument('# A\n## B\nkrótko')).toBe(false);
    expect(isDocument('bez nagłówków '.repeat(100))).toBe(false);
    const blocks = [text(0, '<h1>Raport kwartalny</h1>'), text(1, '<p>…</p>')];
    const [doc] = detectArtifacts(turn(blocks, { text: longDoc }));
    expect(doc?.kind).toBe('markdown');
    expect(doc?.title).toBe('Raport kwartalny');
    expect(doc?.origin).toEqual({ type: 'turn', sessionId: 's1', turnId: 's1:t2' });
    expect(doc?.html_sanitized).toBe('<h1>Raport kwartalny</h1>\n<p>…</p>');
  });

  it('bez artefaktów: wiadomość użytkownika, odpowiedź w trakcie, zwykły kod', () => {
    const html = [code(0, 'html', '&lt;p&gt;x&lt;/p&gt;')];
    expect(detectArtifacts(turn(html, { author: 'user' }))).toEqual([]);
    expect(detectArtifacts(turn(html, { status: 'streaming', text: longDoc }))).toEqual([]);
    expect(detectArtifacts(turn([code(0, 'python', 'print(1)')]))).toEqual([]);
    expect(blockArtifact('t', text(0, '<p>html</p>'))).toBeNull();
  });
});

describe('ucięcie źródła do limitu bajtów', () => {
  it('liczy bajty UTF-8 i nie rozdziela znaków ani par zastępczych', () => {
    expect(clipBytes('abc', 10)).toEqual({ text: 'abc', clipped: false });
    expect(clipBytes('ąę', 3)).toEqual({ text: 'ą', clipped: true });
    expect(clipBytes('a😊b', 4)).toEqual({ text: 'a', clipped: true });
    expect(clipBytes('a😊b', 5)).toEqual({ text: 'a😊', clipped: true });
    const big = 'ż'.repeat(CANVAS_MAX_BYTES);
    const clipped = clipBytes(big, CANVAS_MAX_BYTES);
    expect(clipped.clipped).toBe(true);
    expect(byteSize(clipped.text)).toBe(CANVAS_MAX_BYTES);
  });
});

describe('kanwa obok rozmowy', () => {
  const left = 264;
  const chat = (body: number, l: number, r: number, w: number) => body - l - r - w;

  it('szerokie okno 1440 z oboma panelami: lewy się chowa, rozmowa ≥ 480', () => {
    const dock = canvasDock(1440, left, 460);
    expect(dock).not.toBeNull();
    expect(dock?.hideLeft).toBe(true);
    expect(dock?.hideRight).toBe(false);
    expect(chat(1440, 0, 460, dock?.width ?? 0)).toBeGreaterThanOrEqual(CANVAS_CHAT_MIN);
  });

  it('1600 z oboma panelami mieści się bez zwijania', () => {
    const dock = canvasDock(1600, left, 460);
    expect(dock).toMatchObject({ hideLeft: false, hideRight: false });
    expect(chat(1600, 264, 460, dock?.width ?? 0)).toBeGreaterThanOrEqual(CANVAS_CHAT_MIN);
  });

  it('średnie okno z prawym panelem: prawy chowa się na czas kanwy', () => {
    const dock = canvasDock(1100, null, 360);
    expect(dock?.hideRight).toBe(true);
    expect(chat(1100, 0, 0, dock?.width ?? 0)).toBeGreaterThanOrEqual(CANVAS_CHAT_MIN);
  });

  it('duże okno bez paneli: kanwa najwyżej 760 px; za ciasno — arkusz', () => {
    expect(canvasDock(2560, null, 0)?.width).toBe(760);
    expect(canvasDock(700, null, 0)).toBeNull();
  });
});
