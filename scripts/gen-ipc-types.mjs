#!/usr/bin/env node
// Typy TypeScript łącza rdzeń ↔ interfejs generowane ze schematu JSON wyprowadzonego z typów Rust
// (ADR 0013; plan v6, zadanie B.1). Źródło: packages/schemas/ipc.v1.json (zapisuje je test
// `cargo test -p app-api --test ipc_schema` z UPDATE_SCHEMAS=1). Wynik:
// apps/desktop/ui/src/lib/api/generated/ipc.ts — pliku nie edytuje się ręcznie.
//
// Użycie: node scripts/gen-ipc-types.mjs            (zapis)
//         node scripts/gen-ipc-types.mjs --check    (kod wyjścia 1, gdy plik jest nieaktualny)
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import prettier from 'prettier';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const SCHEMA = join(ROOT, 'packages', 'schemas', 'ipc.v1.json');
const OUT = join(ROOT, 'apps', 'desktop', 'ui', 'src', 'lib', 'api', 'generated', 'ipc.ts');

const schema = JSON.parse(readFileSync(SCHEMA, 'utf8'));
const defs = schema.$defs;

/**
 * Zawężenie po stronie okna (B.1 etap 2): Rust trzyma te pola jako napis, a interfejs zna zamkniętą
 * listę wartości (cztery agentki, przeznaczenie i klasa modelu). Klucz `Typ.pole`; pierwszy element —
 * typ, który musi wyjść ze schematu (inny = rozjazd z Rustem, generator przerywa), drugi — typ w oknie.
 */
const NARROW = {
  'AccountAssignment.agents': ['readonly string[]', 'readonly AgentId[]'],
  'ActivityInfo.agent': ['string', 'AgentId'],
  'AgentRun.agent': ['string', 'AgentId'],
  'AgentState.id': ['string', 'AgentId'],
  'ArtifactInfo.agent': ['string | null', 'AgentId | null'],
  'GpuView.role': ['string', 'GpuRole'],
  'ModelInfo.purpose': ['string', 'ModelPurpose'],
  'ModelInfo.tier': ['string', 'ModelTier'],
  'SendOptions.addressed_to': ['string | null', 'AgentId | null'],
  'TimelineEvent.agent': ['string | null', 'AgentId | null'],
  'Turn.addressed_to': ['string | null', 'AgentId | null'],
  'Turn.author': ['string', 'Author'],
  'VoicePillState.agent': ['string', 'AgentId'],
  'VoiceStatus.agent': ['string', 'AgentId'],
};
/** Słowniki zawężeń (wartości z dokumentacji pól w Rust). */
const VOCABULARY = [
  "import type { AgentId } from '@alfa/ui-kit';",
  '',
  '/** Autor tury: użytkownik albo jedna z agentek. */',
  "export type Author = 'user' | AgentId;",
  '/** Przeznaczenie modelu rozpoznane po nazwie (`ModelInfo.purpose`). */',
  'export type ModelPurpose =',
  "  | 'chat' | 'reasoning' | 'embedding' | 'speech' | 'transcription'",
  "  | 'realtime' | 'image' | 'video' | 'moderation' | 'other';",
  '/** Wielkość modelu (`ModelInfo.tier`). */',
  "export type ModelTier = 'standard' | 'small' | 'tiny' | 'premium';",
  '/** Rola karty przy kilku kartach: modele główne, lekkie zadania albo nieużywana (`GpuView.role`). */',
  "export type GpuRole = 'heavy' | 'light' | 'spare';",
  '',
];
const VOCABULARY_TYPES = ['Author', 'ModelPurpose', 'ModelTier', 'GpuRole'];
/**
 * Pola, które okno uzupełnia na miejscu z delt strumienia (stan `$state`): bez `readonly`, tablice
 * zmienne. Reszta typu zostaje tylko do odczytu.
 */
const MUTABLE_IN_UI = {
  Turn: [
    'approval',
    'blocks',
    'error',
    'heard_prefix',
    'status',
    'text',
    'thinking',
    'tools',
    'truncated',
    'usage',
  ],
};
const narrowed = new Set();

const camel = (name) => name.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
const literal = (value) => JSON.stringify(value);
const key = (name) => (/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? name : literal(name));

function doc(description, indent = '') {
  if (!description) return '';
  const lines = String(description).replace(/\*\//g, '*\\/').split('\n');
  if (lines.length === 1) return `${indent}/** ${lines[0]} */\n`;
  return `${indent}/**\n${lines.map((l) => `${indent} * ${l}`.trimEnd()).join('\n')}\n${indent} */\n`;
}

function refName(ref) {
  const name = ref.replace('#/$defs/', '');
  if (!(name in defs)) throw new Error(`nieznane odwołanie ${ref}`);
  return name;
}

function union(parts) {
  const unique = [...new Set(parts)];
  return unique.length === 1 ? unique[0] : unique.join(' | ');
}

/** Typ TS dla (pod)schematu. */
function type(node) {
  if (node === true || (typeof node === 'object' && Object.keys(node).length === 0))
    return 'unknown';
  if (node.$ref) return refName(node.$ref);
  if (node.const !== undefined) return literal(node.const);
  if (node.enum) return union(node.enum.map(literal));
  if (node.oneOf || node.anyOf) return union((node.oneOf ?? node.anyOf).map(type));
  if (Array.isArray(node.type)) return union(node.type.map((t) => type({ ...node, type: t })));
  switch (node.type) {
    case 'string':
      return 'string';
    case 'integer':
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'null':
      return 'null';
    case 'array': {
      const item = node.items ? type(node.items) : 'unknown';
      return `readonly ${/[ |&]/.test(item) ? `(${item})` : item}[]`;
    }
    case 'object':
      return object(node);
    default:
      throw new Error(`nieobsługiwany schemat: ${JSON.stringify(node).slice(0, 200)}`);
  }
}

function fieldType(owner, name, child) {
  const generated = type(child);
  const rule = owner && NARROW[`${owner}.${name}`];
  if (!rule) return generated;
  if (generated !== rule[0])
    throw new Error(`zawężenie ${owner}.${name}: schemat daje ${generated}, oczekiwano ${rule[0]}`);
  narrowed.add(`${owner}.${name}`);
  return rule[1];
}

function object(node, owner = null) {
  const required = new Set(node.required ?? []);
  const mutable = new Set(MUTABLE_IN_UI[owner] ?? []);
  const fields = Object.entries(node.properties ?? {}).map(([name, child]) => {
    const live = mutable.has(name);
    let field = fieldType(owner, name, child);
    if (live) {
      field = field.replace(/^readonly /, '');
      mutable.delete(name);
    }
    const opt = required.has(name) ? '' : '?';
    return `${doc(child.description)}${live ? '' : 'readonly '}${key(name)}${opt}: ${field};`;
  });
  if (mutable.size > 0)
    throw new Error(`pola zmienne bez pola w schemacie: ${owner}.${[...mutable]}`);
  const extra = node.additionalProperties;
  const record = extra && extra !== true ? `Readonly<Record<string, ${type(extra)}>>` : null;
  if (fields.length === 0) return record ?? 'Readonly<Record<string, unknown>>';
  const body = `{\n${fields.join('\n')}\n}`;
  return record ? `${body} & ${record}` : body;
}

function declaration(name, node) {
  const plain = node.type === 'object' && node.properties && !node.additionalProperties;
  if (plain) return `${doc(node.description)}export interface ${name} ${object(node, name)}`;
  return `${doc(node.description)}export type ${name} = ${type(node)};`;
}

const parts = [
  '// PLIK GENEROWANY — nie edytuj ręcznie. Źródło: typy Rust w `crates/app-api` → schemat',
  '// `packages/schemas/ipc.v1.json` → `node scripts/gen-ipc-types.mjs` (ADR 0013).',
  '',
  ...VOCABULARY,
  `/** Wersja kształtu schematu łącza. */\nexport const IPC_SCHEMA_VERSION = ${schema.version};`,
  '',
];
for (const name of Object.keys(defs).sort()) parts.push(declaration(name, defs[name]), '');
const unused = Object.keys(NARROW).filter((k) => !narrowed.has(k));
if (unused.length > 0) throw new Error(`zawężenia bez pola w schemacie: ${unused.join(', ')}`);

const commands = Object.keys(schema.commands)
  .sort()
  .map((name) => {
    const command = schema.commands[name];
    const args = command.args.map((a) => `readonly ${key(camel(a.name))}: ${type(a.schema)};`);
    const argsType = args.length ? `{ ${args.join(' ')} }` : 'Readonly<Record<string, never>>';
    return `readonly ${name}: { readonly args: ${argsType}; readonly result: ${type(command.result)} };`;
  });
parts.push(
  '/**\n * Komendy rdzenia: nazwa → argumenty (nazwy tak, jak idą z JS — camelCase) i wynik.\n * Wynik `null` to komenda bez wartości zwracanej.\n */',
  `export interface IpcCommands {\n${commands.join('\n')}\n}`,
  '',
  '/** Nazwa komendy rdzenia. */\nexport type IpcCommandName = keyof IpcCommands;',
  '',
  `/** Ładunek kanału \`alfa://events\`. */\nexport type IpcEventFrame = ${type(schema.event_frame)};`,
  `/** Odrzucenie komendy. */\nexport type IpcError = ${type(schema.error)};`,
  '',
  '/** Powód, dla którego polecenie wymaga lokalnej maszyny (B.5, `crates/app-api/data/local-only.json`). */',
  "export type LocalReason = 'system_dialog' | 'asset_preview' | 'host_path' | 'host_desktop';",
  '/**',
  ' * Polecenia zakładające lokalną maszynę (okno systemu, `asset:`, ścieżka hosta, okna i schowek hosta):',
  ' * klient zdalny ich nie dostanie — interfejs po kanale zdalnym chowa ich kontrolki albo używa odpowiednika.',
  ' */',
  `export const LOCAL_ONLY_COMMANDS = {\n${Object.entries(schema.local_only.commands)
    .map(([name, entry]) => `${key(name)}: ${literal(entry.reason)},`)
    .join('\n')}\n} as const satisfies Partial<Record<IpcCommandName, LocalReason>>;`,
  '',
);

const options = (await prettier.resolveConfig(OUT)) ?? {};
const text = await prettier.format(parts.join('\n'), { ...options, parser: 'typescript' });

// ── Bez ręcznych kopii (B.1 etap 2) ──────────────────────────────────────────────────────────────
// Typ łącza istnieje tylko w pliku generowanym; `types*.ts` go przekazują (`export type { X } from
// './generated/ipc'`) i dokładają typy samego interfejsu. Ręczna deklaracja typu o nazwie typu z Rusta
// (albo słownika zawężeń) = błąd generatora — kontrakt nie może się znowu rozdwoić.
const API = join(ROOT, 'apps', 'desktop', 'ui', 'src', 'lib', 'api');
const handwritten = new Map();
for (const file of readdirSync(API)
  .filter((f) => /^types.*\.ts$/.test(f))
  .sort()) {
  const source = readFileSync(join(API, file), 'utf8');
  for (const match of source.matchAll(/^export (?:interface|type) (\w+)/gm))
    if (!handwritten.has(match[1])) handwritten.set(match[1], file);
}
const duplicates = [...handwritten.keys()].filter(
  (name) => name in defs || VOCABULARY_TYPES.includes(name),
);
if (duplicates.length > 0) {
  for (const name of duplicates)
    console.error(
      `gen-ipc-types: ${handwritten.get(name)} deklaruje ${name} — to typ łącza, przekaż go z generated/ipc`,
    );
  process.exit(1);
}

const outputs = [[OUT, text]];
const summary = `${Object.keys(defs).length} typów, ${commands.length} komend, ${narrowed.size} pól zawężonych; typy samego interfejsu: ${handwritten.size}`;
if (process.argv.includes('--check')) {
  const stale = outputs.filter(
    ([path, wanted]) =>
      (existsSync(path) ? readFileSync(path, 'utf8').replace(/\r\n/g, '\n') : '') !== wanted,
  );
  if (stale.length > 0) {
    for (const [path] of stale) console.error(`gen-ipc-types: nieaktualny ${path}`);
    console.error('gen-ipc-types: uruchom `node scripts/gen-ipc-types.mjs`');
    process.exit(1);
  }
  console.log(`gen-ipc-types: aktualne — ${summary}`);
} else {
  mkdirSync(dirname(OUT), { recursive: true });
  for (const [path, wanted] of outputs) writeFileSync(path, wanted);
  console.log(`gen-ipc-types: ${summary}`);
}
