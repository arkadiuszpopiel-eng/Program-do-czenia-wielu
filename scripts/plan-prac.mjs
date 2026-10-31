#!/usr/bin/env node
// Plan prac dla właściciela w jednym pliku HTML: co zrobione, co w toku, co czeka i w jakim stopniu —
// z roadmapy (`docs/plan/03-roadmapa.md`), katalogu funkcji i raportów. Plik generowany, nie edytuj ręcznie.
//
//   node scripts/plan-prac.mjs            → docs/plan-prac.html
//   node scripts/plan-prac.mjs <plik>     → wskazany plik
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = process.argv[2] ?? join(root, 'docs', 'plan-prac.html');
const read = (p) => readFileSync(join(root, p), 'utf8');
const roadmap = read('docs/plan/03-roadmapa.md');

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
/** Markdown z komórki tabeli → bezpieczny HTML (kod, pogrubienie, przekreślenie, linki jako tekst). */
function inline(md) {
  return esc(md)
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/~~([^~]+)~~/g, '<s>$1</s>');
}
const cells = (line) =>
  line
    .trim()
    .replace(/^\||\|$/g, '')
    .split(/(?<!\\)\|/)
    .map((c) => c.trim());

const STATUS = {
  zrobione: { label: 'Zrobione', weight: 1, cls: 'done' },
  'w toku': { label: 'W toku', weight: 0.5, cls: 'prog' },
  'do zrobienia': { label: 'Do zrobienia', weight: 0, cls: 'todo' },
  planowane: { label: 'Planowane', weight: 0, cls: 'todo' },
  czeka: { label: 'Czeka', weight: 0, cls: 'wait' },
  badawcze: { label: 'Badawcze', weight: 0, cls: 'research' },
};
const statusOf = (s) => STATUS[s.toLowerCase()] ?? { label: s || '—', weight: 0, cls: 'todo' };

/** Etapy i podetapy z roadmapy: nagłówki `## Etap …` / `### …` i wiersze tabel z identyfikatorem. */
function stages() {
  const list = [];
  let stage = null;
  let section = null;
  for (const line of roadmap.split('\n')) {
    const h2 = line.match(/^## (Etap .+)$/);
    if (h2) {
      stage = { title: h2[1], sections: [] };
      section = { title: null, items: [] };
      stage.sections.push(section);
      list.push(stage);
      continue;
    }
    if (/^## /.test(line)) {
      stage = null;
      continue;
    }
    const h3 = stage && line.match(/^### (.+)$/);
    if (h3) {
      section = {
        title: h3[1].replace(/\s*\(opis:.*$/, '').replace(/\s*\(spis:.*$/, ''),
        items: [],
      };
      stage.sections.push(section);
      continue;
    }
    if (!stage || !/^\| [A-Z][\w.]* +\|/.test(line)) continue;
    const [id, task, status, date, ...rest] = cells(line);
    if (id === 'ID') continue;
    section.items.push({
      id,
      task,
      status: status ?? '',
      date: date ?? '',
      note: rest.join(' | '),
    });
  }
  return list.map((s) => ({ ...s, sections: s.sections.filter((x) => x.items.length) }));
}

function progress(items) {
  const total = items.length;
  const done = items.filter((i) => statusOf(i.status).weight === 1).length;
  const prog = items.filter((i) => statusOf(i.status).weight === 0.5).length;
  const pct = total ? Math.round(((done + prog * 0.5) / total) * 100) : 0;
  return { total, done, prog, pct };
}

/** Otwarte decyzje właściciela (wiersze nieprzekreślone). */
function decisions() {
  const part = roadmap.split('## Decyzje właściciela')[1]?.split('\n## ')[0] ?? '';
  return part
    .split('\n')
    .filter((l) => l.startsWith('| ') && !/^\| -/.test(l) && !l.startsWith('| Decyzja'))
    .map(cells)
    .filter(([d]) => d && !d.startsWith('~~'))
    .map(([d, blocks]) => ({ d, blocks: blocks ?? '' }));
}

/** Ostatnie wpisy dziennika roadmapy (najnowsze pierwsze). */
function journal(n) {
  const part = roadmap.split('## Dziennik roadmapy')[1] ?? '';
  return part
    .split('\n')
    .filter((l) => /^\| 20\d\d-\d\d-\d\d/.test(l))
    .map(cells)
    .sort((a, b) => b[0].localeCompare(a[0]))
    .slice(0, n);
}

function catalog() {
  const p = 'docs/features/KATALOG.md';
  if (!existsSync(join(root, p))) return null;
  const m = read(p).match(/Razem: (\d+) funkcji, z testem (\d+) \(([\d.,]+) %\), w długu (\d+)/);
  return m && { all: m[1], tested: m[2], pct: m[3], debt: m[4] };
}

const version = read('Cargo.toml').match(/^version = "([^"]+)"/m)?.[1] ?? '?';
const lastReport = readdirSync(join(root, 'docs/plan'))
  .filter((f) => /^\d+-raport-.*\.md$/.test(f))
  .sort((a, b) => parseInt(b) - parseInt(a))[0];
const reportTitle = lastReport
  ? read(`docs/plan/${lastReport}`).split('\n')[0].replace(/^# /, '')
  : '';
const reportOpen = lastReport
  ? (read(`docs/plan/${lastReport}`).split(/^## \d+\. Do decyzji właściciela$/m)[1] ?? '')
      .split('\n## ')[0]
      .split('\n')
      .filter((l) => /^\d+\. /.test(l))
      .map((l) => l.replace(/^\d+\. /, ''))
  : [];

const all = stages();
const everything = all.flatMap((s) => s.sections.flatMap((x) => x.items));
const total = progress(everything);
const cat = catalog();
const now = new Date().toLocaleString('pl-PL', { dateStyle: 'long', timeStyle: 'short' });

const bar = (p) =>
  `<div class="bar" role="img" aria-label="${p.pct} %"><span class="d" style="width:${(p.done / (p.total || 1)) * 100}%"></span><span class="p" style="width:${(p.prog / (p.total || 1)) * 100}%"></span></div>`;
const chip = (s) => `<span class="chip ${statusOf(s).cls}">${esc(statusOf(s).label)}</span>`;
const short = (s, n) => (s.length > n ? `${s.slice(0, n).replace(/\s+\S*$/, '')} …` : s);

function item(i) {
  const note = i.note && i.note !== '—' ? i.note : '';
  const body = note
    ? note.length > 220
      ? `<details><summary>${inline(short(note, 220))}</summary><p>${inline(note)}</p></details>`
      : `<p>${inline(note)}</p>`
    : '';
  return `<li class="it" data-s="${statusOf(i.status).cls}"><div class="row"><span class="id">${esc(i.id)}</span><span class="t">${inline(i.task)}</span>${chip(i.status)}${i.date && i.date !== '—' ? `<span class="dt">${esc(i.date)}</span>` : ''}</div>${body}</li>`;
}

function stage(s) {
  const items = s.sections.flatMap((x) => x.items);
  const p = progress(items);
  const secs = s.sections
    .map(
      (x) =>
        `${x.title ? `<h4>${inline(x.title)} <small>${progress(x.items).pct} %</small></h4>` : ''}<ul>${x.items.map(item).join('')}</ul>`,
    )
    .join('');
  return `<details class="stage"${p.prog || (p.done && p.done < p.total) ? ' open' : ''}><summary><span class="st">${inline(s.title)}</span><span class="nums">${p.done}/${p.total} zrobione${p.prog ? `, ${p.prog} w toku` : ''}</span><span class="pct">${p.pct} %</span>${bar(p)}</summary>${secs}</details>`;
}

const counts = Object.entries(
  everything.reduce(
    (m, i) => ((m[statusOf(i.status).label] = (m[statusOf(i.status).label] ?? 0) + 1), m),
    {},
  ),
)
  .map(([k, v]) => `<li><b>${v}</b> ${esc(k.toLowerCase())}</li>`)
  .join('');

const html = `<!doctype html>
<html lang="pl"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Plan prac Alfy</title>
<style>
:root{--bg:#f6f7f9;--card:#fff;--fg:#1b1f24;--mut:#5b6470;--line:#e2e5ea;--done:#1f8a4c;--prog:#c27a00;--wait:#7a5cc2;--todo:#8a929c;--res:#2a7ab8;--accent:#0b63ce}
@media (prefers-color-scheme:dark){:root:not([data-theme=light]){--bg:#121417;--card:#1b1e23;--fg:#e8eaed;--mut:#9aa3ad;--line:#2c3138;--done:#3fbf73;--prog:#e5a33a;--wait:#a68cf0;--todo:#8f98a3;--res:#5aa8e6;--accent:#5aa2ff}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 "Segoe UI Variable","Segoe UI",system-ui,sans-serif}
main{max-width:1080px;margin:0 auto;padding:24px 16px 64px}h1{font-size:26px;margin:0 0 4px}h2{font-size:19px;margin:32px 0 12px}h4{margin:16px 0 6px;font-size:14px;color:var(--mut)}h4 small{font-weight:400}
.sub{color:var(--mut);margin:0 0 20px}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px}
.card{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px}.big{font-size:28px;font-weight:600}.lbl{color:var(--mut);font-size:13px}
.bar{display:flex;height:8px;border-radius:4px;background:var(--line);overflow:hidden;flex:1 1 160px;min-width:120px}.bar .d{background:var(--done)}.bar .p{background:var(--prog)}
.counts{list-style:none;padding:0;margin:8px 0 0;display:flex;flex-wrap:wrap;gap:4px 14px;font-size:13px;color:var(--mut)}.counts b{color:var(--fg)}
.stage{background:var(--card);border:1px solid var(--line);border-radius:10px;margin:10px 0;padding:0 16px}.stage>summary{cursor:pointer;display:flex;flex-wrap:wrap;align-items:center;gap:8px 14px;padding:14px 0;list-style:none}
.stage>summary::-webkit-details-marker{display:none}.st{font-weight:600;flex:1 1 280px}.nums{color:var(--mut);font-size:13px}.pct{font-weight:600;min-width:44px;text-align:right}
ul{list-style:none;margin:0;padding:0 0 12px}.it{border-top:1px solid var(--line);padding:8px 0}.row{display:flex;flex-wrap:wrap;gap:6px 10px;align-items:baseline}
.id{font:12px "Cascadia Mono",Consolas,monospace;color:var(--mut);min-width:52px}.t{flex:1 1 300px}.dt{font-size:12px;color:var(--mut)}
.it p,.it details{margin:4px 0 0 62px;color:var(--mut);font-size:13px}.it summary{cursor:pointer}code{font:12px "Cascadia Mono",Consolas,monospace;background:var(--line);padding:0 4px;border-radius:4px}
.chip{font-size:12px;padding:1px 8px;border-radius:999px;border:1px solid currentColor;white-space:nowrap}.chip.done{color:var(--done)}.chip.prog{color:var(--prog)}.chip.wait{color:var(--wait)}.chip.todo{color:var(--todo)}.chip.research{color:var(--res)}
.filters{display:flex;flex-wrap:wrap;gap:8px;margin:8px 0}.filters button{font:inherit;font-size:13px;background:var(--card);color:var(--fg);border:1px solid var(--line);border-radius:999px;padding:4px 12px;cursor:pointer}.filters button[aria-pressed=true]{border-color:var(--accent);color:var(--accent)}
ol.dec,ul.log{padding-left:20px;list-style:decimal}ul.log{list-style:disc}ol.dec li,ul.log li{margin:6px 0}.when{color:var(--mut);font-size:13px;margin-right:6px}
@media (max-width:600px){.it p,.it details{margin-left:0}}
</style></head><body><main>
<h1>Plan prac Alfy</h1>
<p class="sub">Stan na ${esc(now)} · wersja programu <b>${esc(version)}</b> · ostatni raport: ${esc(reportTitle)}</p>
<div class="grid">
<div class="card"><div class="lbl">Cały plan (${total.total} zadań)</div><div class="big">${total.pct} %</div>${bar(total)}<ul class="counts">${counts}</ul></div>
${cat ? `<div class="card"><div class="lbl">Funkcje programu z testem AI</div><div class="big">${esc(cat.pct)} %</div><div class="lbl">${esc(cat.tested)} z ${esc(cat.all)} funkcji · bez testu: ${esc(cat.debt)}</div></div>` : ''}
${all
  .slice(0, 3)
  .map((s) => {
    const p = progress(s.sections.flatMap((x) => x.items));
    return `<div class="card"><div class="lbl">${inline(s.title)}</div><div class="big">${p.pct} %</div>${bar(p)}</div>`;
  })
  .join('')}
</div>
${reportOpen.length ? `<h2>Czeka na Twoją decyzję (z ostatniego raportu)</h2><ol class="dec">${reportOpen.map((l) => `<li>${inline(l)}</li>`).join('')}</ol>` : ''}
<h2>Ostatnie prace</h2><ul class="log">${journal(8)
  .map(([d, c]) => `<li><span class="when">${esc(d)}</span>${inline(short(c, 360))}</li>`)
  .join('')}</ul>
<h2>Etapy i zadania</h2>
<div class="filters" role="group" aria-label="Pokaż zadania">
<button aria-pressed="true" data-f="all">Wszystkie</button><button aria-pressed="false" data-f="done">Zrobione</button><button aria-pressed="false" data-f="prog">W toku</button><button aria-pressed="false" data-f="wait">Czeka</button><button aria-pressed="false" data-f="todo">Planowane</button>
</div>
${all.map(stage).join('\n')}
<h2>Decyzje właściciela, które blokują zadania</h2>
<ol class="dec">${decisions()
  .map(
    ({ d, blocks }) =>
      `<li>${inline(d)}${blocks ? ` <span class="when">— blokuje: ${inline(blocks)}</span>` : ''}</li>`,
  )
  .join('')}</ol>
<p class="sub">Źródło: ${esc(relative(root, join(root, 'docs/plan/03-roadmapa.md')).replace(/\\/g, '/'))} · plik generuje <code>node scripts/plan-prac.mjs</code></p>
</main>
<script>
document.querySelectorAll('.filters button').forEach((b) => b.addEventListener('click', () => {
  document.querySelectorAll('.filters button').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
  const f = b.dataset.f;
  document.querySelectorAll('.it').forEach((i) => { i.hidden = f !== 'all' && i.dataset.s !== f && !(f === 'todo' && i.dataset.s === 'research'); });
  document.querySelectorAll('.stage').forEach((s) => { if (f !== 'all') s.open = !!s.querySelector('.it:not([hidden])'); s.hidden = !s.querySelector('.it:not([hidden])'); });
}));
</script>
</body></html>
`;
writeFileSync(out, html);
console.log(`plan-prac: ${everything.length} zadań, ${total.pct} % → ${relative(root, out)}`);
