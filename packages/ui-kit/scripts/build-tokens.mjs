#!/usr/bin/env node
// Generuje src/tokens.css (zmienne --alfa-*) i src/tokens.ts z src/tokens/tokens.json.
// Uruchom: pnpm --filter @alfa/ui-kit build:tokens. CI pilnuje `git diff --exit-code`.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const tokens = JSON.parse(readFileSync(join(pkg, 'src/tokens/tokens.json'), 'utf8'));

const HEADER =
  '/* WYGENEROWANE z src/tokens/tokens.json przez scripts/build-tokens.mjs — nie edytuj. */';
const px = (n) => `${n}px`;
const lines = (obj, fn) => Object.entries(obj).map(([k, v]) => fn(k, v));

/** Deklaracje niezależne od motywu. */
function baseDecls() {
  const out = [];
  out.push(`--alfa-font-sans: ${tokens.font.family.sans};`);
  out.push(`--alfa-font-mono: ${tokens.font.family.mono};`);
  out.push(...lines(tokens.font.size, (k, v) => `--alfa-font-size-${k}: ${px(v)};`));
  out.push(...lines(tokens.font.lineHeight, (k, v) => `--alfa-leading-${k}: ${v};`));
  out.push(...lines(tokens.font.weight, (k, v) => `--alfa-weight-${k}: ${v};`));
  out.push(...lines(tokens.space, (k, v) => `--alfa-space-${k}: ${px(v)};`));
  out.push(...lines(tokens.radius, (k, v) => `--alfa-radius-${k}: ${px(v)};`));
  out.push(...lines(tokens.size, (k, v) => `--alfa-size-${kebab(k)}: ${px(v)};`));
  out.push(...lines(tokens.motion.duration, (k, v) => `--alfa-duration-${k}: ${v}ms;`));
  out.push(...lines(tokens.motion.easing, (k, v) => `--alfa-ease-${kebab(k)}: ${v};`));
  return out;
}

/** Deklaracje zależne od motywu (light | dark). */
function themeDecls(theme) {
  const out = [];
  out.push(`color-scheme: ${theme};`);
  out.push(...lines(tokens.color.neutral[theme], (k, v) => `--alfa-color-${kebab(k)}: ${v};`));
  for (const [id, a] of Object.entries(tokens.color.agent)) {
    out.push(`--alfa-agent-${id}: ${a[theme]};`);
    out.push(`--alfa-agent-${id}-soft: ${theme === 'light' ? a.softLight : a.softDark};`);
  }
  out.push(...lines(tokens.color.semantic, (k, v) => `--alfa-color-${k}: ${v[theme]};`));
  out.push(...lines(tokens.color.risk, (k, v) => `--alfa-color-risk-${k}: ${v[theme]};`));
  out.push(...lines(tokens.elevation[theme], (k, v) => `--alfa-shadow-${k}: ${v};`));
  // W ciemnym motywie elewacja = jaśniejsze tło + obramowanie (bez cieni).
  out.push(`--alfa-elevation-border: 1px solid var(--alfa-color-border);`);
  return out;
}

/** Wymuszony wysoki kontrast (Windows): kolory systemowe zamiast palety. */
function forcedColorsDecls() {
  return [
    '--alfa-color-bg: Canvas;',
    '--alfa-color-surface: Canvas;',
    '--alfa-color-surface2: Canvas;',
    '--alfa-color-surface3: Canvas;',
    '--alfa-color-border: CanvasText;',
    '--alfa-color-border-strong: CanvasText;',
    '--alfa-color-text: CanvasText;',
    '--alfa-color-text-muted: CanvasText;',
    '--alfa-color-text-subtle: CanvasText;',
    '--alfa-color-text-on-accent: HighlightText;',
    '--alfa-color-focus: Highlight;',
    ...Object.keys(tokens.color.agent).flatMap((id) => [
      `--alfa-agent-${id}: Highlight;`,
      `--alfa-agent-${id}-soft: Canvas;`,
    ]),
    ...Object.keys(tokens.color.semantic).map((k) => `--alfa-color-${k}: CanvasText;`),
    ...Object.keys(tokens.color.risk).map((k) => `--alfa-color-risk-${k}: CanvasText;`),
    '--alfa-shadow-1: none;',
    '--alfa-shadow-2: none;',
    '--alfa-shadow-3: none;',
  ];
}

function reducedMotionDecls() {
  return Object.keys(tokens.motion.duration).map((k) => `--alfa-duration-${k}: 0ms;`);
}

function block(selector, decls, indent = '') {
  return `${indent}${selector} {\n${decls.map((d) => `${indent}  ${d}`).join('\n')}\n${indent}}`;
}

function kebab(s) {
  return s.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
}

const css = [
  HEADER,
  block(':root', [...baseDecls(), ...themeDecls('light')]),
  `@media (prefers-color-scheme: dark) {\n${block(':root:not([data-theme="light"])', themeDecls('dark'), '  ')}\n}`,
  block(':root[data-theme="dark"]', themeDecls('dark')),
  `@media (forced-colors: active) {\n${block(':root', forcedColorsDecls(), '  ')}\n}`,
  `@media (prefers-reduced-motion: reduce) {\n${block(':root', reducedMotionDecls(), '  ')}\n}`,
  block(':root[data-motion="off"]', reducedMotionDecls()),
  '',
].join('\n\n');

const agentIds = Object.keys(tokens.color.agent);
const ts = `${HEADER.replace('/*', '//').replace(' */', '')}
/* eslint-disable */

export const tokens = ${JSON.stringify(tokens, null, 2)} as const;

export type Tokens = typeof tokens;
export type Theme = 'light' | 'dark';
export type AgentId = ${agentIds.map((id) => `'${id}'`).join(' | ')};
export type SemanticColor = ${Object.keys(tokens.color.semantic)
  .map((k) => `'${k}'`)
  .join(' | ')};
export type RiskLevel = ${Object.keys(tokens.color.risk)
  .map((k) => `'${k}'`)
  .join(' | ')};
export type FontSize = keyof Tokens['font']['size'];
export type Space = keyof Tokens['space'];
export type Radius = keyof Tokens['radius'];

export const agentIds: readonly AgentId[] = [${agentIds.map((id) => `'${id}'`).join(', ')}];

export interface AgentMeta {
  readonly id: AgentId;
  readonly name: string;
  readonly glyph: string;
}

export const agents: Readonly<Record<AgentId, AgentMeta>> = {
${agentIds.map((id) => `  ${id}: { id: '${id}', name: '${tokens.color.agent[id].name}', glyph: '${tokens.color.agent[id].glyph}' },`).join('\n')}
};

/** Zmienna CSS akcentu agentki, np. \`var(--alfa-agent-alfa)\`. */
export const agentVar = (id: AgentId): string => \`var(--alfa-agent-\${id})\`;
/** Zmienna CSS delikatnego tła agentki. */
export const agentSoftVar = (id: AgentId): string => \`var(--alfa-agent-\${id}-soft)\`;
/** Kolor agentki jako HEX dla danego motywu (np. do Canvas 2D). */
export const agentHex = (id: AgentId, theme: Theme): string => tokens.color.agent[id][theme];
`;

writeFileSync(join(pkg, 'src/tokens.css'), css);
writeFileSync(join(pkg, 'src/tokens.ts'), ts);
console.log('build-tokens: zapisano src/tokens.css i src/tokens.ts');
