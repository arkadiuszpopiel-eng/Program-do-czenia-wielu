#!/usr/bin/env node
// Sprawdza kontrast WCAG 2.x dla tokenów (PLAN.md §14.3):
//  - tekst główny ≥ 7:1, pomocniczy ≥ 4,5:1 na tle i powierzchniach,
//  - każdy kolor agentki i semantyczny ≥ 4,5:1 (gdy niesie tekst) i ≥ 3:1 (UI, WCAG 1.4.11)
//    na tle jasnym (wariant light) i ciemnym (wariant dark), także na powierzchniach 1–2.
// Wypisuje tabelę, exit 1 gdy próg nie jest spełniony.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const tokens = JSON.parse(readFileSync(join(here, '../src/tokens/tokens.json'), 'utf8'));

const TEXT_MIN = 4.5;
const UI_MIN = 3;
const PRIMARY_TEXT_MIN = 7;

function srgbToLinear(c) {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}
function luminance(hex) {
  const h = hex.replace('#', '');
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
  return 0.2126 * srgbToLinear(r) + 0.7152 * srgbToLinear(g) + 0.0722 * srgbToLinear(b);
}
export function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const rows = [];
let failures = 0;
function check(group, name, theme, fg, bgName, bg, min) {
  const ratio = contrast(fg, bg);
  const ok = ratio >= min;
  if (!ok) failures++;
  rows.push({ group, name, theme, fg, bg: `${bgName} ${bg}`, ratio: ratio.toFixed(2), min, ok });
}

for (const theme of ['light', 'dark']) {
  const n = tokens.color.neutral[theme];
  const backgrounds = [
    ['bg', n.bg],
    ['surface', n.surface],
    ['surface2', n.surface2],
  ];
  for (const [bgName, bg] of backgrounds) {
    check('neutral', 'text', theme, n.text, bgName, bg, PRIMARY_TEXT_MIN);
    check('neutral', 'textMuted', theme, n.textMuted, bgName, bg, TEXT_MIN);
    check('neutral', 'textSubtle', theme, n.textSubtle, bgName, bg, TEXT_MIN);
    check('neutral', 'focus', theme, n.focus, bgName, bg, UI_MIN);
    check('neutral', 'borderStrong', theme, n.borderStrong, bgName, bg, 1.5);
    for (const [id, a] of Object.entries(tokens.color.agent)) {
      check('agent', `${a.name} (tekst)`, theme, a[theme], bgName, bg, TEXT_MIN);
      check('agent', `${a.name} (UI)`, theme, a[theme], bgName, bg, UI_MIN);
      void id;
    }
    for (const [k, v] of Object.entries(tokens.color.semantic)) {
      check('semantic', `${k} (tekst)`, theme, v[theme], bgName, bg, TEXT_MIN);
    }
    for (const [k, v] of Object.entries(tokens.color.risk)) {
      check('risk', `${k} (tekst)`, theme, v[theme], bgName, bg, TEXT_MIN);
    }
  }
  // Tekst na akcencie (np. przycisk główny w kolorze agentki).
  for (const a of Object.values(tokens.color.agent)) {
    check('agent', `${a.name} ← textOnAccent`, theme, n.textOnAccent, 'agent', a[theme], TEXT_MIN);
    check(
      'agent',
      `${a.name} soft ← text`,
      theme,
      n.text,
      'soft',
      theme === 'light' ? a.softLight : a.softDark,
      PRIMARY_TEXT_MIN,
    );
  }
}

// Rozróżnialność kolorów agentek między sobą (pomocniczo; nie blokuje).
const pairs = [];
const ids = Object.keys(tokens.color.agent);
for (const theme of ['light', 'dark']) {
  for (let i = 0; i < ids.length; i++)
    for (let j = i + 1; j < ids.length; j++) {
      const a = tokens.color.agent[ids[i]][theme];
      const b = tokens.color.agent[ids[j]][theme];
      pairs.push(`${theme}: ${ids[i]} vs ${ids[j]} = ${contrast(a, b).toFixed(2)}`);
    }
}

const pad = (s, n) => String(s).padEnd(n);
console.log(
  pad('grupa', 9) +
    pad('token', 26) +
    pad('motyw', 7) +
    pad('kolor', 9) +
    pad('tło', 20) +
    pad('ratio', 7) +
    pad('min', 5) +
    'wynik',
);
for (const r of rows) {
  console.log(
    pad(r.group, 9) +
      pad(r.name, 26) +
      pad(r.theme, 7) +
      pad(r.fg, 9) +
      pad(r.bg, 20) +
      pad(r.ratio, 7) +
      pad(r.min, 5) +
      (r.ok ? 'OK' : 'FAIL'),
  );
}
console.log('\nRozróżnialność par agentek (informacyjnie):\n  ' + pairs.join('\n  '));
console.log(`\ncheck-contrast: ${rows.length} sprawdzeń, ${failures} niespełnionych`);
process.exit(failures ? 1 : 0);
