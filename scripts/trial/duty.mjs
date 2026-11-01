#!/usr/bin/env node
// Próba gotowych obowiązków agentek na lokalnym modelu, z automatyczną oceną (docs/plan/24 §2.6).
// Działa na INSTANCJI TESTOWEJ (`scripts/trial/profile.sh start --model`), nigdy na profilu
// właściciela. Dla każdego wzoru: wyzwalacz ręczny z poleceniem wzoru → „uruchom teraz” →
// czekanie na koniec zadania → ocena, czy raport nie wychodzi poza dane programu.
//
//   node scripts/trial/duty.mjs [--only morning,costs] [--minutes 10]
// Wynik: target/verify/trial/raport.md + raport.json; kod wyjścia 1, gdy którakolwiek próba czerwona.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { formProblems, judge } from './grounding.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const require = createRequire(join(root, 'apps', 'desktop', 'ui', 'package.json'));
const { chromium } = require('@playwright/test');

const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const port = process.env.ALFA_TRIAL_PORT ?? '9333';
const minutes = Number(option('--minutes', '10'));
const only = option('--only', null)?.split(',') ?? null;

/** Polecenia wzorów prosto z tekstów programu — próba sprawdza to, co zobaczy użytkownik. */
function dutyGoals() {
  const text = readFileSync(join(root, 'apps/desktop/ui/src/lib/i18n/pl-tasks.ts'), 'utf8');
  const goals = {};
  for (const match of text.matchAll(/'duty\.(\w+)\.goal':\s*'([^']+)'/g))
    goals[match[1]] = match[2];
  return goals;
}

const pad = (n) => String(n).padStart(2, '0');
const localStamp = (iso) => {
  const d = new Date(iso);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
};
const money = (m) => `${Math.floor(Math.abs(m.minor) / 100)},${pad(Math.abs(m.minor) % 100)}`;

/** Dane, które program podaje agentce — odtworzone z tych samych poleceń rdzenia co arkusz faktów. */
function factsText(tasks, triggers, costs, goal, startedMs) {
  // Arkusz faktów ma godzinę startu zadania — dozwolona jest każda minuta od startu do teraz.
  const stamps = [];
  for (let t = startedMs - 60_000; t <= Date.now() + 60_000; t += 60_000) {
    stamps.push(localStamp(new Date(t).toISOString()));
  }
  // Ta sama data w zapisie z kropkami oraz imiona obsady — agentka może ich użyć bez zmyślania.
  const now = new Date();
  const dotted = `${pad(now.getDate())}.${pad(now.getMonth() + 1)}.${now.getFullYear()}`;
  const cast = '„alfa” „beta” „gama” „delta” „Alfa” „Beta” „Gama” „Delta”';
  const lines = [goal, ...stamps, dotted, cast, `${tasks.length} ${triggers.length}`];
  for (const t of tasks) lines.push(`„${t.title}” ${t.steps} ${t.max_steps} ${money(t.cost)}`);
  for (const t of triggers) {
    lines.push(`„${t.name}” ${t.next_fire_at ? localStamp(t.next_fire_at) : ''}`);
  }
  if (costs) {
    lines.push(`${money(costs.day)} ${money(costs.month)} ${money(costs.limit.monthly)}`);
    // Okresy z arkusza („7 dni”) i zerowa historia kosztów profilu próbnego.
    lines.push('7 14 0,00');
    const share =
      costs.limit.monthly.minor > 0 ? (costs.month.minor / costs.limit.monthly.minor) * 100 : 0;
    lines.push(`${share.toFixed(0)} ${share.toFixed(1)} ${share.toFixed(2)} 100`);
  }
  return lines.join('\n');
}

/**
 * Treść stron wskazanych w poleceniu (np. cenniki dostawców) jako dodatkowe dane dla oceny: liczba
 * z raportu odczytana ze strony nie jest „spoza danych”. Ceny OpenRouter są w USD za token, a raport
 * podaje je za 1 mln tokenów — dlatego także wartości × 1 000 000. Strona, której nie da się pobrać,
 * nie dodaje niczego (raport ma wtedy napisać, że jej nie sprawdził).
 */
async function sourcesText(goal) {
  const urls = [...new Set(goal.match(/https?:\/\/[^\s,;)]+/g) ?? [])];
  const parts = [];
  for (const url of urls) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(20_000) });
      const body = (await res.text()).slice(0, 3_000_000).replace(/<[^>]+>/g, ' ');
      const scaled = (body.match(/\d+\.\d+(?:e-?\d+)?/g) ?? [])
        .map((n) => Number(n) * 1_000_000)
        .filter((n) => Number.isFinite(n) && n > 0 && n < 100_000)
        .map((n) => String(Math.round(n * 1000) / 1000));
      parts.push(body, scaled.join(' '));
    } catch {
      // Strona niedostępna dla oceny — bez danych z niej.
    }
  }
  return parts.join('\n');
}

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 20_000 });
const main = browser
  .contexts()
  .flatMap((c) => c.pages())
  .find((p) => new URL(p.url()).pathname === '/');
if (!main) throw new Error('Brak okna głównego instancji testowej.');
const invoke = (command, payload) =>
  main.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).catch((e) => {
        throw new Error(JSON.stringify(e));
      }),
    [command, payload ?? {}],
  );

const about = await invoke('updates_about');
const results = [];
for (const [id, goal] of Object.entries(dutyGoals())) {
  if (only && !only.includes(id)) continue;
  // Godzina w nazwie: kolejny bieg na tym samym profilu nie trafia na istniejący wyzwalacz.
  const name = `Próba obowiązku: ${id} ${new Date().toTimeString().slice(0, 8)}`;
  const trigger = await invoke('triggers_create', {
    draft: {
      name,
      kind: { kind: 'manual' },
      title: name,
      goal,
      agent: 'alfa',
      bridge: null,
      respect_dnd: false,
    },
  });
  const run = await invoke('triggers_fire_now', { triggerId: trigger.id });
  const started = Date.now();
  let task = null;
  for (;;) {
    task = (await invoke('tasks_list')).find((t) => t.id === run.task_id) ?? null;
    if (task?.state === 'done' || Date.now() - started > minutes * 60_000) break;
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  }
  const seconds = Math.round((Date.now() - started) / 1000);
  const finished = task?.state === 'done' && task.result === 'succeeded';
  const report = task?.summary ?? '';
  const otherTasks = (await invoke('tasks_list')).filter((t) => t.id !== run.task_id);
  const triggers = await invoke('triggers_list');
  const facts = factsText(
    otherTasks,
    triggers,
    await invoke('costs_summary', { sessionId: null }),
    goal,
    started,
  );
  const outside = finished
    ? [
        ...judge(
          report,
          [facts, await sourcesText(goal)],
          Math.max(otherTasks.length, triggers.length),
        ),
        ...formProblems(report),
      ]
    : [];
  // Zgoda Sędziego Cieni (np. pobranie strony) w próbie bez właściciela nie przyjdzie — to stan
  // środowiska, nie błąd agentki; raport mówi o nim wprost i nie liczy go jako zielony.
  const waited = /ApprovalWait/.test(task?.error ?? '');
  const state = waited
    ? 'czeka na zgodę'
    : !finished || outside.length > 0
      ? 'CZERWONY'
      : 'zielony';
  const why = waited
    ? 'zadanie poprosiło o zgodę Sędziego Cieni (np. na pobranie strony); w próbie nikt jej nie daje'
    : !finished
      ? `zadanie nie skończyło się powodzeniem w ${minutes} min (stan: ${task?.state ?? 'brak'}, wynik: ${task?.result ?? '—'}, błąd: ${task?.error ?? '—'})`
      : outside.join('; ');
  results.push({ id, state, seconds, why, report });
  console.log(`— ${id}: ${state} (${seconds} s) ${why}`);
}
await browser.close();

const out = join(root, 'target', 'verify', 'trial');
mkdirSync(out, { recursive: true });
const lines = [
  '# Próba obowiązków agentek na lokalnym modelu',
  '',
  `Wersja programu: ${about.version} · bieg: ${new Date().toISOString()}`,
  '',
  '| Obowiązek | Wynik | Czas | Wyjścia poza dane / powód |',
  '| --- | --- | --- | --- |',
  ...results.map((r) => `| ${r.id} | ${r.state} | ${r.seconds} s | ${r.why || '—'} |`),
  '',
  ...results.flatMap((r) => [`## ${r.id}`, '', '```', r.report || '(brak raportu)', '```', '']),
];
writeFileSync(join(out, 'raport.md'), `${lines.join('\n')}\n`);
writeFileSync(
  join(out, 'raport.json'),
  `${JSON.stringify({ version: about.version, results }, null, 2)}\n`,
);
console.log(`Raport: ${join(out, 'raport.md')}`);
process.exit(results.some((r) => r.state === 'CZERWONY') ? 1 : 0);
