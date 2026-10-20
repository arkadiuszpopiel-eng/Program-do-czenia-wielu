// Atrapa `canvas_render` (C2.8): Markdown przez atrapę renderera, HTML/SVG jako dokument dla
// izolowanej ramki — uproszczona sanitizacja (bez skryptów, handlerów, ramek i adresów zewnętrznych).
// Prawdziwą białą listę ma rdzeń (`lib_markdown::artifact_document`); w obu przypadkach ramka
// ma `sandbox` bez skryptów, więc atrapa nie jest jedyną warstwą.
import { CANVAS_MAX_BYTES, byteSize } from '../../logic/canvas';
import type { CanvasKind } from '../types-files';
import { renderBlocks } from './render';

const BASE_STYLE =
  ':root{color-scheme:light dark}' +
  "body{margin:0;padding:16px;font:15px/1.5 system-ui,'Segoe UI',sans-serif;overflow-wrap:anywhere}" +
  'img,svg{max-width:100%;height:auto}table{border-collapse:collapse}';

function roughSanitize(source: string): string {
  return source
    .replace(/<\?xml[^>]*\?>/gi, '')
    .replace(/<!doctype[^>]*>/gi, '')
    .replace(/<(script|iframe|object|embed|title|foreignObject)\b[\s\S]*?<\/\1\s*>/gi, '')
    .replace(/<\/?(html|head|body|meta|link|base|form|input)\b[^>]*>/gi, '')
    .replace(/\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, '')
    .replace(/\s(?:xlink:)?(?:href|src)\s*=\s*("(?!#)[^"]*"|'(?!#)[^']*')/gi, '')
    .replace(/url\(\s*['"]?(?!#)/gi, 'blocked(')
    .replace(/@import/gi, '@blocked');
}

/** Znacznik w źródle, przy którym atrapa odrzuca render (stan błędu kanwy w testach E2E). */
export const FAKE_RENDER_FAILURE = 'data-atrapa-blad-renderu';

export function fakeCanvasRender(kind: CanvasKind, source: string, lang = 'pl'): string {
  if (!source.trim()) throw new Error('Artefakt jest pusty.');
  if (source.includes(FAKE_RENDER_FAILURE)) {
    throw new Error('Atrapa: rdzeń nie przygotował podglądu artefaktu.');
  }
  if (byteSize(source) > CANVAS_MAX_BYTES) {
    throw new Error(
      `Artefakt jest za duży do podglądu w kanwie (limit ${CANVAS_MAX_BYTES / 1024} KB).`,
    );
  }
  if (kind === 'markdown') {
    return renderBlocks(source, false)
      .map((b) => b.html_sanitized)
      .join('\n');
  }
  return (
    `<!doctype html><html lang="${/^[a-z0-9-]{1,35}$/i.test(lang) ? lang : 'pl'}">` +
    '<head><meta charset="utf-8">' +
    `<style>${BASE_STYLE}</style></head><body>${roughSanitize(source)}</body></html>`
  );
}
