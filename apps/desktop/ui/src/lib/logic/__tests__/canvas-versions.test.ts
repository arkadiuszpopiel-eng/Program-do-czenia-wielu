import { describe, expect, it } from 'vitest';
import type { RenderedBlock } from '../../api/types';
import { detectArtifacts, type CanvasItem } from '../canvas';
import {
  DIFF_CONTEXT,
  DIFF_MAX_LINES,
  artifactVersions,
  diffChunks,
  diffLines,
  versionKey,
} from '../canvas-versions';

// Blok kodu tak, jak go zwraca rdzeń (treść escapowana).
const code = (index: number, lang: string, html: string): RenderedBlock => ({
  index,
  kind: 'code',
  lang,
  html_sanitized: `<pre><code data-lang="${lang}">${html}</code></pre>\n`,
  closed: true,
});

const page = (title: string, body: string) =>
  `&lt;html&gt;&lt;head&gt;&lt;title&gt;${title}&lt;/title&gt;&lt;/head&gt;&lt;body&gt;${body}&lt;/body&gt;&lt;/html&gt;`;

const turn = (id: string, blocks: RenderedBlock[], author = 'gama') => ({
  id,
  session_id: 's1',
  author,
  status: 'complete',
  text: 'odpowiedź',
  blocks,
});

const first = (t: ReturnType<typeof turn>): CanvasItem => {
  const item = detectArtifacts(t)[0];
  if (!item) throw new Error('brak artefaktu');
  return item;
};

describe('versionKey', () => {
  it('ten sam rodzaj i tytuł bez wielkości liter i nadmiaru spacji', () => {
    const a = first(turn('t1', [code(0, 'html', page('Karta  projektu', 'a'))]));
    const b = first(turn('t2', [code(0, 'html', page('karta projektu', 'b'))]));
    expect(versionKey(a)).toBe(versionKey(b));
  });

  it('bez tytułu albo z pliku — brak historii', () => {
    const svg = first(turn('t1', [code(0, 'svg', '&lt;svg&gt;&lt;/svg&gt;')]));
    expect(svg.title).toBeNull();
    expect(versionKey(svg)).toBeNull();
    const file: CanvasItem = {
      key: 'f1',
      kind: 'markdown',
      title: 'notatki.md',
      source: '# A',
      origin: { type: 'file', artifactId: 'f1' },
    };
    expect(versionKey(file)).toBeNull();
  });

  it('inny rodzaj o tym samym tytule to inny artefakt', () => {
    const html = first(turn('t1', [code(0, 'html', page('Plan', 'a'))]));
    const md = first(turn('t2', [code(0, 'markdown', '# Plan')]));
    expect(versionKey(html)).not.toBe(versionKey(md));
  });
});

describe('artifactVersions', () => {
  const v1 = turn('t2', [code(0, 'html', page('Karta projektu', 'pierwsza'))]);
  const other = turn('t4', [code(0, 'html', page('Inna strona', 'x'))]);
  const v2 = turn('t6', [
    code(0, 'svg', '&lt;svg&gt;&lt;/svg&gt;'),
    code(1, 'html', page('Karta projektu', 'druga')),
  ]);
  const user = turn('t7', [code(0, 'html', page('Karta projektu', 'od użytkownika'))], 'user');
  const path = [v1, other, v2, user];

  it('kolejne odpowiedzi z tym samym artefaktem, od najstarszej', () => {
    const current = detectArtifacts(v2)[1];
    if (!current) throw new Error('brak');
    const versions = artifactVersions(path, current);
    expect(versions.map((v) => v.key)).toEqual(['t2:0', 't6:1']);
    // Pokazywany artefakt to ten sam obiekt (porównanie w kanwie po kluczu i po referencji).
    expect(versions[1]).toBe(current);
  });

  it('wiadomości użytkownika nie są wersjami', () => {
    const versions = artifactVersions(path, first(v1));
    expect(versions.every((v) => !v.key.startsWith('t7'))).toBe(true);
  });

  it('dwa bloki o tym samym tytule w jednej odpowiedzi — pokazywany ma pierwszeństwo', () => {
    const twice = turn('t8', [
      code(0, 'html', page('Karta projektu', 'a')),
      code(1, 'html', page('Karta projektu', 'b')),
    ]);
    const second = detectArtifacts(twice)[1];
    if (!second) throw new Error('brak');
    const versions = artifactVersions([v1, twice], second);
    expect(versions.map((v) => v.key)).toEqual(['t2:0', 't8:1']);
  });

  it('artefakt spoza ścieżki albo bez tytułu — tylko on sam', () => {
    const svg = first(turn('t9', [code(0, 'svg', '&lt;svg&gt;&lt;/svg&gt;')]));
    expect(artifactVersions(path, svg)).toEqual([svg]);
    const gone = first(turn('t10', [code(0, 'html', page('Karta projektu', 'usunięta'))]));
    expect(artifactVersions(path, gone)).toEqual([gone]);
  });
});

describe('diffLines', () => {
  it('wiersze dodane, usunięte i wspólne', () => {
    const diff = diffLines('a\nb\nc', 'a\nB\nc\nd');
    expect(diff).toEqual([
      { op: 'same', text: 'a' },
      { op: 'del', text: 'b' },
      { op: 'add', text: 'B' },
      { op: 'same', text: 'c' },
      { op: 'add', text: 'd' },
    ]);
  });

  it('końce wierszy CRLF nie są zmianą', () => {
    expect(diffLines('a\r\nb', 'a\nb')?.every((l) => l.op === 'same')).toBe(true);
  });

  it('za dużo wierszy — brak porównania', () => {
    const big = 'x\n'.repeat(DIFF_MAX_LINES + 1);
    expect(diffLines(big, 'x')).toBeNull();
    expect(diffLines('x', big)).toBeNull();
  });
});

describe('diffChunks', () => {
  it('zwija długie ciągi bez zmian i liczy zmiany', () => {
    const before = Array.from({ length: 20 }, (_, i) => `w${i}`).join('\n');
    const after = before.replace('w10', 'W10');
    const diff = diffLines(before, after);
    if (!diff) throw new Error('brak');
    const result = diffChunks(diff);
    expect(result.added).toBe(1);
    expect(result.removed).toBe(1);
    expect(result.chunks[0]).toEqual({
      type: 'skip',
      count: 10 - DIFF_CONTEXT,
    });
    const lines = result.chunks[1];
    expect(lines?.type === 'lines' && lines.lines.length).toBe(2 * DIFF_CONTEXT + 2);
    expect(result.chunks[2]).toEqual({
      type: 'skip',
      count: 20 - 11 - DIFF_CONTEXT,
    });
  });

  it('bez zmian — jeden pominięty ciąg', () => {
    const diff = diffLines('a\nb', 'a\nb');
    if (!diff) throw new Error('brak');
    expect(diffChunks(diff)).toEqual({
      added: 0,
      removed: 0,
      chunks: [{ type: 'skip', count: 2 }],
    });
  });
});
