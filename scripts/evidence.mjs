#!/usr/bin/env node
// Rejestr dowodów (audyt 2026-10-14, A-15): jedna tabela z raportów `scripts/verify.mjs`
// (`target/verify/<wersja>/raport-<poziom>.json`) — kiedy, na jakim sprzęcie, z którego commita,
// jakim poleceniem i z jakim wynikiem. Plik `docs/DOWODY.md` jest generowany; nie pisze się go ręcznie.
// Logi etapów zostają lokalnie w `target/verify/` (poza repozytorium).
//
//   node scripts/evidence.mjs            zapisuje docs/DOWODY.md
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const verifyDir = join(root, 'target', 'verify');
const LEVELS = ['wersja', 'sprzet', 'szybki'];

const semver = (v) => v.split('.').map(Number);
const newer = (a, b) => {
  const [x, y] = [semver(a), semver(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return y[i] - x[i];
  return 0;
};

const runs = [];
const versions = existsSync(verifyDir)
  ? readdirSync(verifyDir).filter((d) => /^\d+\.\d+\.\d+$/.test(d))
  : [];
for (const version of versions.sort(newer)) {
  for (const level of LEVELS) {
    const file = join(verifyDir, version, `raport-${level}.json`);
    if (!existsSync(file)) continue;
    const report = JSON.parse(readFileSync(file, 'utf8'));
    runs.push({ ...report, version, level, mtime: statSync(file).mtime.toISOString() });
  }
}

const cell = (text) => String(text ?? '').replace(/\|/g, '\\|');
/** Czas zakończenia biegu w strefie Warszawy (raport zapisuje UTC). */
const when = (r) =>
  new Intl.DateTimeFormat('sv-SE', {
    timeZone: 'Europe/Warsaw',
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(new Date(r.finished ?? r.mtime));
const hardware = (r) =>
  r.hardware
    ? `${r.hardware.cpu}, ${r.hardware.ram_gb} GB RAM${r.hardware.gpus?.length ? `, ${r.hardware.gpus.join(' + ')}` : ''}`
    : 'brak w raporcie (starszy format raportu)';
const commit = (r) =>
  r.commit ? `\`${r.commit.slice(0, 8)}\`${r.dirty ? ' + zmiany niezatwierdzone' : ''}` : '—';
const verdictOf = (r) => {
  if (r.verdict) return r.verdict;
  const states = r.results.map((x) => x.state);
  if (states.includes('CZERWONY')) return 'CZERWONY';
  return states.includes('nieuruchomiony') ? 'NIEKOMPLETNY' : 'ZIELONY';
};
const counts = (r) => {
  const n = (s) => r.results.filter((x) => x.state === s).length;
  return `${n('zielony')} / ${n('CZERWONY')} / ${n('nieuruchomiony')}`;
};

const lines = [
  '# Rejestr dowodów',
  '',
  'Plik generowany (`node scripts/evidence.mjs`) z raportów `scripts/verify.mjs`. Każdy wiersz to jeden bieg:',
  'kiedy, na jakim sprzęcie, z którego commita i z jakim wynikiem. Logi etapów leżą lokalnie w',
  '`target/verify/<wersja>/` (poza repozytorium). Zielony etap dowodzi tylko tego, co ten etap sprawdza —',
  'testy na atrapach nie są pomiarem na sprzęcie; kryteria akceptacji i ich progi są w [`STATUS.md`](STATUS.md).',
  '',
];
for (const level of LEVELS) {
  const mine = runs.filter((r) => r.level === level);
  if (!mine.length) continue;
  lines.push(`## Poziom „${level}”`, '');
  lines.push(
    '| Wersja | Zakończony | Commit | Sprzęt | Wynik | Etapy zielone / czerwone / nieuruchomione |',
  );
  lines.push('| --- | --- | --- | --- | --- | --- |');
  for (const r of mine) {
    lines.push(
      `| ${r.version} | ${when(r)} | ${commit(r)} | ${cell(hardware(r))} | ${verdictOf(r)} | ${counts(r)} |`,
    );
  }
  const last = mine[0];
  lines.push('', `### Ostatni bieg: ${last.version} (${when(last)})`, '');
  lines.push('| Etap | Wynik | Liczby / powód | Polecenie | Log |');
  lines.push('| --- | --- | --- | --- | --- |');
  for (const s of last.results) {
    lines.push(
      `| ${cell(s.title)} | ${s.state} | ${cell(s.why ?? s.numbers ?? '')} | ${s.command ? `\`${cell(s.command)}\`` : '—'} | ${s.log ? `\`target/verify/${last.version}/${s.log}\`` : '—'} |`,
    );
  }
  lines.push('');
}
writeFileSync(join(root, 'docs', 'DOWODY.md'), `${lines.join('\n')}\n`);
console.log(`evidence: ${runs.length} biegów → docs/DOWODY.md`);
