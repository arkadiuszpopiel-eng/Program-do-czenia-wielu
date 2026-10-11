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

function object(node) {
  const required = new Set(node.required ?? []);
  const fields = Object.entries(node.properties ?? {}).map(
    ([name, child]) =>
      `${doc(child.description)}readonly ${key(name)}${required.has(name) ? '' : '?'}: ${type(child)};`,
  );
  const extra = node.additionalProperties;
  const record = extra && extra !== true ? `Readonly<Record<string, ${type(extra)}>>` : null;
  if (fields.length === 0) return record ?? 'Readonly<Record<string, unknown>>';
  const body = `{\n${fields.join('\n')}\n}`;
  return record ? `${body} & ${record}` : body;
}

function declaration(name, node) {
  const plain = node.type === 'object' && node.properties && !node.additionalProperties;
  if (plain) return `${doc(node.description)}export interface ${name} ${object(node)}`;
  return `${doc(node.description)}export type ${name} = ${type(node)};`;
}

const parts = [
  '// PLIK GENEROWANY — nie edytuj ręcznie. Źródło: typy Rust w `crates/app-api` → schemat',
  '// `packages/schemas/ipc.v1.json` → `node scripts/gen-ipc-types.mjs` (ADR 0013).',
  '',
  `/** Wersja kształtu schematu łącza. */\nexport const IPC_SCHEMA_VERSION = ${schema.version};`,
  '',
];
for (const name of Object.keys(defs).sort()) parts.push(declaration(name, defs[name]), '');

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
);

const options = (await prettier.resolveConfig(OUT)) ?? {};
const text = await prettier.format(parts.join('\n'), { ...options, parser: 'typescript' });

// ── Zgodność typów pisanych ręcznie (`types*.ts`) z generowanymi ───────────────────────────────
// Dopóki interfejs korzysta z typów pisanych ręcznie, ten plik pilnuje, żeby nie rozjechały się
// z Rustem: błąd typów w `generated/compat.ts` (widzi go `pnpm check`) = rozjazd kontraktu.
const API = join(ROOT, 'apps', 'desktop', 'ui', 'src', 'lib', 'api');
const COMPAT = join(API, 'generated', 'compat.ts');
/** Interfejs zawęża tu napis z Rusta do listy wartości (`AgentId`, `ModelPurpose`, `Author`, rola karty). */
const NARROWED_IN_UI = new Set([
  'Account',
  'AccountAssignment',
  'ActivityInfo',
  'AgentRun',
  'AgentRunDetail',
  'AgentState',
  'AlfaEvent',
  'ArtifactInfo',
  'DeviceProfile',
  'EventFrame',
  'EventsResume',
  'ModelInfo',
  'SendOptions',
  'TestReport',
  'TimelineEvent',
  'Turn',
  'TurnsSnapshot',
  'VoicePillState',
  'VoiceStatus',
]);
const handwritten = new Map();
for (const file of readdirSync(API)
  .filter((f) => /^types.*\.ts$/.test(f))
  .sort()) {
  const source = readFileSync(join(API, file), 'utf8');
  for (const match of source.matchAll(/^export (?:interface|type) (\w+)/gm))
    if (!handwritten.has(match[1])) handwritten.set(match[1], file.replace(/\.ts$/, ''));
}
const shared = [...handwritten.keys()].filter((name) => name in defs).sort();
const modules = [...new Set(shared.map((name) => handwritten.get(name)))].sort();
const alias = (module) => `H${modules.indexOf(module)}`;
const checks = shared.flatMap((name) => {
  const hand = `${alias(handwritten.get(name))}.${name}`;
  const out = [];
  if (!NARROWED_IN_UI.has(name)) out.push(`  Expect<Fits<G.${name}, ${hand}>>,`);
  out.push(`  Expect<Fits<${hand}, G.${name}>>,`);
  return out;
});
const compatSource = [
  '// PLIK GENEROWANY — nie edytuj ręcznie (`node scripts/gen-ipc-types.mjs`).',
  '// Każdy wiersz sprawdza w czasie kompilacji, że typ pisany ręcznie (`types*.ts`) zgadza się',
  '// z typem generowanym z Rusta: `Fits<G.X, H.X>` — to, co wysyła rdzeń, pasuje do typu',
  '// interfejsu; `Fits<H.X, G.X>` — typ interfejsu ma wszystkie pola, które zna rdzeń.',
  '// Błąd w tym pliku = rozjazd kontraktu: popraw typ ręczny (albo DTO w Rust i wygeneruj ponownie).',
  "import type * as G from './ipc';",
  ...modules.map((module) => `import type * as ${alias(module)} from '../${module}';`),
  '',
  'type Fits<A, B> = [A] extends [B] ? true : false;',
  'type Expect<T extends true> = T;',
  '',
  `/** ${shared.length} typów wspólnych, ${checks.length} sprawdzeń. */`,
  `export type IpcCompat = [\n${checks.join('\n')}\n];`,
  '',
].join('\n');
const compat = await prettier.format(compatSource, { ...options, parser: 'typescript' });
const onlyRust = Object.keys(defs).filter((name) => !handwritten.has(name));
const onlyUi = [...handwritten.keys()].filter((name) => !(name in defs));

const outputs = [
  [OUT, text],
  [COMPAT, compat],
];
const summary = `${Object.keys(defs).length} typów, ${commands.length} komend; wspólnych z typami ręcznymi: ${shared.length} (${checks.length} sprawdzeń), tylko w Rust: ${onlyRust.length}, tylko w interfejsie: ${onlyUi.length}`;
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
