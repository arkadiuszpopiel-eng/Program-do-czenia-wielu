// Minimalny tokenizer składni (bez zależności) uruchamiany w Web Workerze po zamknięciu bloku kodu.
// Zwraca zakresy, nie HTML — DOM buduje wątek główny przez `textContent` (bez innerHTML).

export type TokenKind = 'kw' | 'str' | 'num' | 'com';
export type TokenRange = readonly [start: number, end: number, kind: TokenKind];

const KEYWORDS = new Set(
  (
    'abstract as async await break case catch class const continue crate def default do elif else enum ' +
    'export extends false fn for from function if impl import in interface let loop match mod mut new ' +
    'None null pass pub raise return self Self static struct super switch this throw trait True true try ' +
    'type typeof undefined use var where while with yield SELECT FROM WHERE GROUP BY ORDER AS AND OR NOT ' +
    'INSERT INTO VALUES UPDATE SET DELETE JOIN ON LIMIT SUM COUNT echo fi then esac'
  ).split(' '),
);

const HASH_COMMENT = new Set([
  'py',
  'python',
  'sh',
  'bash',
  'shell',
  'ps1',
  'powershell',
  'toml',
  'yaml',
  'yml',
  'r',
]);
const DASH_COMMENT = new Set(['sql', 'lua', 'hs']);

export function tokenize(code: string, lang: string | null): TokenRange[] {
  const out: TokenRange[] = [];
  const l = (lang ?? '').toLowerCase();
  const hash = HASH_COMMENT.has(l);
  const dash = DASH_COMMENT.has(l);
  let i = 0;
  while (i < code.length) {
    const c = code[i] ?? '';
    const next = code[i + 1] ?? '';
    const lineComment =
      (c === '/' && next === '/') || (hash && c === '#') || (dash && c === '-' && next === '-');
    if (lineComment) {
      const end = code.indexOf('\n', i);
      const stop = end < 0 ? code.length : end;
      out.push([i, stop, 'com']);
      i = stop;
    } else if (c === '/' && next === '*') {
      const end = code.indexOf('*/', i + 2);
      const stop = end < 0 ? code.length : end + 2;
      out.push([i, stop, 'com']);
      i = stop;
    } else if (c === '"' || c === "'" || c === '`') {
      let j = i + 1;
      while (j < code.length && code[j] !== c && code[j] !== '\n') j += code[j] === '\\' ? 2 : 1;
      const stop = Math.min(code.length, j + 1);
      out.push([i, stop, 'str']);
      i = stop;
    } else if (/[0-9]/.test(c) && !/[\w$]/.test(code[i - 1] ?? '')) {
      const match = /^[0-9][0-9_.xXa-fA-F]*/.exec(code.slice(i));
      const stop = i + (match?.[0].length ?? 1);
      out.push([i, stop, 'num']);
      i = stop;
    } else if (/[A-Za-z_]/.test(c)) {
      const match = /^[A-Za-z_][\w]*/.exec(code.slice(i));
      const word = match?.[0] ?? c;
      if (KEYWORDS.has(word)) out.push([i, i + word.length, 'kw']);
      i += word.length;
    } else i++;
  }
  return out;
}
