#!/usr/bin/env node
// Buduje docs/html/ z dokumentów planu (Markdown → samodzielne strony HTML otwierane dwuklikiem).
// Źródłem są pliki MD — HTML nie jest edytowany ręcznie. Strona główna liczy postęp etapów z kolumny
// „Stan” w docs/plan/03-roadmapa.md. Wynik jest deterministyczny (bez dat budowy), więc ponowne
// uruchomienie bez zmian w źródłach nie zmienia plików.
//
// Użycie: node scripts/docs-html.mjs [--check | --verify]
//   --check   buduje strony i kończy się błędem, gdy w źródłach jest zerwany odnośnik (także
//             kotwica) albo wiersz roadmapy z nieznanym stanem.
//   --verify  niczego nie zapisuje: porównuje wynik z docs/html na dysku i kończy się błędem,
//             gdy zatwierdzona kopia jest nieaktualna (bramka i CI).
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, posix, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Marked } from 'marked';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = join(root, 'docs', 'html');
const verify = process.argv.includes('--verify');
const check = verify || process.argv.includes('--check');
const rel = (p) => relative(root, p).split('\\').join('/');
const list = (dir, filter) =>
  readdirSync(join(root, dir))
    .filter(filter)
    .sort()
    .map((f) => `${dir}/${f}`);

/** Moduły planu v6 — ich SPEC-e trafiają do HTML (reszta SPEC-ów zostaje w MD). */
const PLANNED = [
  'platform-select',
  'remote-gateway',
  'device-pairing',
  'remote-relay',
  'host-headless',
  'web-panel',
  'offline-client',
  'platform-android',
  'platform-linux',
];

const groups = [
  ['Plan v6', list('docs/plan', (f) => f.endsWith('.md') && f !== 'README.md')],
  ['Stan i historia', ['docs/STATUS.md', 'docs/DOWODY.md', 'docs/KONTYNUACJA.md', 'CHANGELOG.md']],
  ['Decyzje (ADR)', ['docs/ADR/README.md', ...list('docs/ADR', (f) => /^\d.*\.md$/.test(f))]],
  [
    'Nowe moduły (SPEC)',
    PLANNED.map((m) => `docs/modules/${m}/SPEC.md`).filter((p) => existsSync(join(root, p))),
  ],
];
const HOME = 'docs/plan/README.md';
const sources = [HOME, ...groups.flatMap(([, files]) => files)];

const slugOf = (src) =>
  src === HOME
    ? 'index'
    : src
        .replace(/^docs\//, '')
        .replace(/\.md$/, '')
        .replace(/\/SPEC$/, '')
        .replace(/\/README$/, '')
        .replace(/[^A-Za-z0-9]+/g, '-')
        .toLowerCase();
const pageOf = new Map(sources.map((src) => [src, `${slugOf(src)}.html`]));

const esc = (s) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', '#39': "'" };
/** Sam tekst z HTML-a: bez znaczników i z rozwiniętymi encjami (ponowne `esc` nie koduje ich dwa razy). */
const plain = (html) =>
  html
    .replace(/<[^>]+>/g, '')
    .replace(/&(amp|lt|gt|quot|#39);/g, (_, name) => ENTITIES[name])
    .trim();
const anchor = (text) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/ł/g, 'l')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

const STATES = ['zrobione', 'w toku', 'do zrobienia', 'czeka', 'planowane', 'badawcze'];
const broken = [];
/** Odnośniki z kotwicą do sprawdzenia po wyrenderowaniu wszystkich stron: [źródło, href, strona, kotwica]. */
const anchors = [];
const SCHEMES = /^(https?|mailto):/i;

/** Czy ścieżka istnieje dokładnie w tej pisowni (Windows nie rozróżnia wielkości liter, Linux i GitHub — tak). */
function existsExact(target) {
  const full = join(root, target);
  if (!existsSync(full)) return false;
  return rel(realpathSync.native(full)) === target;
}

/** Odnośnik względny: do strony HTML, gdy cel jest w zestawie; inaczej do pliku w repozytorium. */
function rewrite(href, src) {
  if (href.startsWith('#')) {
    anchors.push([src, href, src, href.slice(1)]);
    return href;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
    // Tylko zwykłe adresy; `javascript:` i podobne nie mają czego szukać w dokumentacji.
    if (!SCHEMES.test(href)) broken.push(`${src}: ${href} (niedozwolony rodzaj adresu)`);
    return SCHEMES.test(href) ? href : '#';
  }
  const [rawPath, hash = ''] = href.split('#');
  let path = rawPath;
  try {
    path = decodeURI(rawPath);
  } catch {
    // Zostaje zapis ze źródła.
  }
  const target = posix.normalize(posix.join(posix.dirname(src), path)).replace(/\/$/, '');
  const suffix = hash ? `#${hash}` : '';
  // Odnośnik do samego katalogu wynikowego (np. z README planu) — strona obok.
  if (target.startsWith('docs/html/')) {
    const file = posix.basename(target);
    if (![...pageOf.values()].includes(file)) broken.push(`${src}: ${href}`);
    return file + suffix;
  }
  if (!existsExact(target)) broken.push(`${src}: ${href}`);
  let pageSrc = pageOf.has(target) ? target : null;
  if (!pageSrc && pageOf.has(`${target}/README.md`)) pageSrc = `${target}/README.md`;
  if (pageSrc) {
    if (hash) anchors.push([src, href, pageSrc, hash]);
    return `${pageOf.get(pageSrc)}${suffix}`;
  }
  return `${posix.relative('docs/html', target)}${suffix}`;
}

function render(src) {
  const toc = [];
  const used = new Set();
  const marked = new Marked({ gfm: true });
  marked.use({
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens);
        let id = anchor(plain(html)) || 'sekcja';
        while (used.has(id)) id += '-2';
        used.add(id);
        if (depth === 2) toc.push([id, plain(html)]);
        return `<h${depth} id="${id}">${html}</h${depth}>\n`;
      },
      link({ href, tokens }) {
        return `<a href="${esc(rewrite(href, src))}">${this.parser.parseInline(tokens)}</a>`;
      },
      tablecell({ tokens, header, align }) {
        const html = this.parser.parseInline(tokens);
        const tag = header ? 'th' : 'td';
        const state = STATES.indexOf(plain(html));
        const cls = state >= 0 ? ` class="st st${state}"` : '';
        const style = align ? ` style="text-align:${align}"` : '';
        return `<${tag}${cls}${style}>${html}</${tag}>`;
      },
      table(token) {
        const row = (cells) => `<tr>${cells.map((c) => this.tablecell(c)).join('')}</tr>`;
        const head = row(token.header);
        const body = token.rows.map(row).join('\n');
        return `<div class="tbl"><table><thead>${head}</thead><tbody>${body}</tbody></table></div>\n`;
      },
    },
  });
  const md = readFileSync(join(root, src), 'utf8');
  const body = marked.parse(md);
  const title = plain(/<h1[^>]*>(.*?)<\/h1>/s.exec(body)?.[1] ?? src);
  return { body, title, toc, ids: used };
}

/** Postęp etapów: sekcje `## Etap X — …` i wartości kolumny „Stan” w ich tabelach. */
function progress() {
  const md = readFileSync(join(root, 'docs/plan/03-roadmapa.md'), 'utf8');
  const stages = [];
  const problems = [];
  let stage = null;
  let stateCol = -1;
  for (const line of md.split(/\r?\n/)) {
    const head = /^## Etap ([A-Z]) — (.+)$/.exec(line);
    if (head) {
      stage = { id: head[1], name: head[2], counts: STATES.map(() => 0) };
      stages.push(stage);
      stateCol = -1;
      continue;
    }
    if (/^## Etap/.test(line)) problems.push(`nagłówek etapu w nieznanym zapisie: ${line}`);
    if (/^## /.test(line)) stage = null;
    // Koniec tabeli: następna zaczyna od własnego nagłówka.
    if (!line.startsWith('|')) stateCol = -1;
    if (!stage || !line.startsWith('|')) continue;
    const cells = line
      .split('|')
      .slice(1, -1)
      .map((c) => c.trim());
    if (cells.includes('Stan')) stateCol = cells.indexOf('Stan');
    else if (stateCol < 0 || cells.every((c) => /^:?-+:?$/.test(c))) continue;
    else if (STATES.includes(cells[stateCol])) stage.counts[STATES.indexOf(cells[stateCol])] += 1;
    // Literówka albo dopisek w kolumnie „Stan” po cichu wypadałby z postępu.
    else problems.push(`etap ${stage.id}: nieznany stan „${cells[stateCol]}” (${cells[0]})`);
  }
  const updated = /Ostatnia aktualizacja: \*\*([^*]+)\*\*/.exec(md)?.[1] ?? '';
  return { stages, updated, problems };
}

const roadmap = progress();

function dashboard() {
  const { stages, updated } = roadmap;
  const version = /^version\s*=\s*"([^"]+)"/m.exec(
    readFileSync(join(root, 'Cargo.toml'), 'utf8'),
  )?.[1];
  const rows = stages
    .map((s) => {
      const total = s.counts.reduce((a, b) => a + b, 0);
      const bar = s.counts
        .map((n, i) =>
          n ? `<span class="st${i}" style="flex:${n}" title="${STATES[i]}: ${n}"></span>` : '',
        )
        .join('');
      const done = total ? Math.round((s.counts[0] / total) * 100) : 0;
      return `<tr><th scope="row">${s.id}</th><td>${esc(s.name)}</td><td><div class="bar" role="img" aria-label="zrobione ${s.counts[0]} z ${total}">${bar}</div></td><td class="num">${s.counts[0]} / ${total}</td><td class="num">${done}%</td></tr>`;
    })
    .join('\n');
  const legend = STATES.map((s, i) => `<span class="chip st st${i}">${s}</span>`).join(' ');
  return `<section class="dash" aria-labelledby="postep"><h2 id="postep">Postęp etapów</h2>
<p class="meta">Wersja programu <strong>${esc(version ?? '?')}</strong> · roadmapa z dnia <strong>${esc(updated)}</strong> · liczone z <a href="${pageOf.get('docs/plan/03-roadmapa.md')}">roadmapy</a></p>
<div class="tbl"><table><thead><tr><th>Etap</th><th>Zakres</th><th>Zadania</th><th>Zrobione</th><th>%</th></tr></thead><tbody>
${rows}
</tbody></table></div><p class="legend">${legend}</p></section>\n`;
}

const css = readFileSync(join(root, 'scripts', 'docs-html.css'), 'utf8');
const script = `(()=>{const k='alfa-docs-theme',r=document.documentElement;let t=null;try{t=localStorage.getItem(k)}catch{}
if(t)r.dataset.theme=t;if(matchMedia('(min-width: 901px)').matches)document.getElementById('navd').open=true;document.getElementById('theme').addEventListener('click',()=>{const d=(r.dataset.theme||(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'))==='dark'?'light':'dark';
r.dataset.theme=d;try{localStorage.setItem(k,d)}catch{}})})()`;

function nav(current) {
  const item = (src, title) =>
    `<li><a href="${pageOf.get(src)}"${src === current ? ' aria-current="page"' : ''}>${esc(title)}</a></li>`;
  const sections = groups
    .map(
      ([name, files]) =>
        `<h2>${esc(name)}</h2><ul>${files.map((src) => item(src, pages.get(src).title)).join('')}</ul>`,
    )
    .join('');
  // Na wąskim ekranie spis jest zwinięty (skrypt strony rozwija go na szerokim).
  return `<nav aria-label="Dokumenty"><details id="navd"><summary>Spis dokumentów</summary><ul>${item(HOME, 'Start')}</ul>${sections}</details></nav>`;
}

const pages = new Map(sources.map((src) => [src, render(src)]));
for (const [src, href, pageSrc, hash] of anchors) {
  if (!pages.get(pageSrc)?.ids.has(hash)) broken.push(`${src}: ${href} (brak takiej kotwicy)`);
}
// Wszystko powstaje w pamięci, zanim cokolwiek zniknie z dysku.
const board = dashboard();
const output = new Map();
for (const [src, page] of pages) {
  const toc =
    page.toc.length > 2
      ? `<aside class="toc" aria-label="Na tej stronie"><ul>${page.toc.map(([id, text]) => `<li><a href="#${id}">${esc(text)}</a></li>`).join('')}</ul></aside>`
      : '';
  const home = src === HOME;
  const body = home ? page.body.replace(/(<\/h1>\n)/, `$1${board}`) : page.body;
  const html = `<!doctype html>
<html lang="pl">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(page.title)} — Alfa</title>
<!-- Plik generowany: node scripts/docs-html.mjs ze źródła ${src}. Nie edytuj ręcznie. -->
<style>${css}</style>
</head>
<body>
<header><a class="brand" href="index.html">Alfa — plan i stan</a><button id="theme" type="button">Jasny / ciemny</button></header>
<div class="layout">
${nav(src)}
<main>${toc}<article>${body}</article>
<footer>Źródło: <code>${src}</code> · strona generowana, zmiany wprowadzaj w pliku MD.</footer></main>
</div>
<script>${script}</script>
</body>
</html>
`;
  output.set(pageOf.get(src), html);
}

const problems = [
  ...broken.map((b) => `zerwany odnośnik — ${b}`),
  ...roadmap.problems.map((p) => `roadmapa — ${p}`),
];
if (verify) {
  // Porównanie z kopią na dysku (bez znaków CR, które dodaje Git na Windows).
  const text = (s) => s.replace(/\r\n/g, '\n');
  const onDisk = existsSync(outDir) ? readdirSync(outDir).filter((f) => f.endsWith('.html')) : [];
  for (const [file, html] of output) {
    const path = join(outDir, file);
    if (!existsSync(path)) problems.push(`nieaktualne — brak strony ${file}`);
    else if (text(readFileSync(path, 'utf8')) !== text(html))
      problems.push(`nieaktualne — ${file} różni się od źródła`);
  }
  for (const file of onDisk) {
    if (!output.has(file)) problems.push(`nieaktualne — zbędna strona ${file}`);
  }
} else {
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  for (const [file, html] of output) writeFileSync(join(outDir, file), html);
}

console.log(
  `docs-html: ${pages.size} stron ${verify ? 'sprawdzonych z' : '→'} ${rel(outDir)}; problemy: ${problems.length}`,
);
for (const p of problems) console.log(`  ${p}`);
if (verify && problems.some((p) => p.startsWith('nieaktualne')))
  console.log('  Uruchom: node scripts/docs-html.mjs i zatwierdź docs/html.');
if (check && problems.length) process.exit(1);
