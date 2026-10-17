#!/usr/bin/env node
// Jedno sprawdzenie — jeden raport (docs/plan/24 §2.1). Uruchamia etapy wybranego poziomu po
// kolei, pilnuje miejsca na dysku i zapisuje `target/verify/<wersja>/raport.md` oraz `raport.json`.
// Raport wymienia także etapy NIEURUCHOMIONE, z powodem — lista „nie sprawdziłem” nie powstaje
// z pamięci.
//
//   node scripts/verify.mjs szybki [pakiet…]   formatowanie, linter i testy podanych pakietów, interfejs
//   node scripts/verify.mjs wersja             cały projekt + przeglądarka + prawdziwa aplikacja + pokrycie łącza
//   node scripts/verify.mjs sprzet [--pulpit] [--dzwiek]   testy sprzętowe + próby na modelu
//
// Przed startem: `source scripts/portable-env.sh`. Kod wyjścia 1, gdy którykolwiek etap jest czerwony;
// 3, gdy żaden nie jest czerwony, ale któryś się nie uruchomił (brak narzędzia, miejsca) — bieg
// niekompletny. Poziom „wersja” obejmuje te same bramki co CI (audyt 2026-10-14, A-09).
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, statfsSync, writeFileSync } from 'node:fs';
import { cpus, freemem, totalmem } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Ile przeglądarek testowych naraz: każda zajmuje ok. 1,5 GB. Przy małej wolnej pamięci (inne programy
 * otwarte obok) cztery naraz dławią maszynę i testy padają na przekroczeniu czasu w losowych
 * miejscach (2026-10-14: 2,6 GB wolnej pamięci → 11–14 losowych czerwonych). `PW_WORKERS` nadpisuje.
 */
function e2eWorkers() {
  if (process.env.PW_WORKERS) return process.env.PW_WORKERS;
  const freeGb = freemem() / 1024 ** 3;
  return String(Math.max(1, Math.min(4, Math.floor(freeGb / 1.5))));
}

/**
 * Powłoka etapów. Na Windows samo `bash` trafia najpierw na `C:\Windows\System32\bash.exe` (launcher
 * WSL) — bez dystrybucji WSL każdy etap kończył się w 0 s (2026-10-17, komputer stacjonarny). Bierzemy
 * Git Bash: `ALFA_BASH` z otoczenia albo `usr\bin\bash.exe` z instalacji Git for Windows — ten nie
 * przestawia PATH. `bin\bash.exe` dokleja na początek `/usr/bin` i budowa OpenSSL brała okrojony perl
 * z Git zamiast Strawberry Perl z `alfa-narzedzia` (`portable-env`).
 */
function gitBash() {
  if (process.env.ALFA_BASH) return process.env.ALFA_BASH;
  if (process.platform !== 'win32') return 'bash';
  const roots = [process.env.ProgramFiles, 'C:\\Program Files'].filter(Boolean);
  const found = roots
    .flatMap((r) => [join(r, 'Git', 'usr', 'bin', 'bash.exe'), join(r, 'Git', 'bin', 'bash.exe')])
    .find((p) => existsSync(p));
  return found ?? 'bash';
}
const BASH = gitBash();

/**
 * Otoczenie etapów: katalog Git Bash (`bash`, `xargs`, `sed`…) wstawiony do PATH tuż przed
 * `System32` — za przenośnymi programami z `portable-env` (Strawberry Perl, cargo, node mają
 * pierwszeństwo), ale przed launcherem WSL (`#!/usr/bin/env bash` w `npx` trafiał na
 * `System32\bash.exe`). Na Windows klucz bywa `Path`.
 */
function stageEnv() {
  const env = { ...process.env, ...cargoEnv };
  if (process.platform === 'win32' && BASH !== 'bash') {
    const key = Object.keys(env).find((k) => k.toUpperCase() === 'PATH') ?? 'PATH';
    const parts = (env[key] ?? '').split(';').filter(Boolean);
    const system = parts.findIndex((p) => /\\system32\\?$/i.test(p));
    parts.splice(system < 0 ? parts.length : system, 0, dirname(BASH));
    env[key] = parts.join(';');
  }
  return env;
}

// Kody kolorów terminala (ESC[…m) — bez znaku sterującego w literale wyrażenia.
const ANSI = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, 'g');
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const [level, ...rest] = process.argv.slice(2);
if (!['szybki', 'wersja', 'sprzet'].includes(level)) {
  console.error(
    'Użycie: node scripts/verify.mjs szybki [pakiet…] | wersja | sprzet [--pulpit] [--dzwiek]',
  );
  process.exit(2);
}
const version = JSON.parse(
  readFileSync(join(root, 'apps/desktop/ui/package.json'), 'utf8'),
).version;
const out = join(root, 'target', 'verify', version);
mkdirSync(out, { recursive: true });
const ui = join(root, 'apps', 'desktop', 'ui');
const packages = rest.filter((a) => !a.startsWith('--'));
const flags = rest.filter((a) => a.startsWith('--'));
const cargoEnv = {
  CARGO_INCREMENTAL: '0',
  CARGO_PROFILE_DEV_DEBUG: '0',
  CARGO_PROFILE_TEST_DEBUG: '0',
};
const pkgArgs = packages.flatMap((p) => ['-p', p]);
const SHELL = 'apps/desktop/src-tauri/Cargo.toml';

/** Etap: `id`, opis, polecenie (`sh -c` w Git Bash), katalog, minimalne wolne miejsce w GB. */
const STAGES = {
  szybki: [
    [
      'format',
      'Formatowanie Rust',
      `git ls-files '*.rs' | xargs -n 200 rustfmt --edition 2024 --check`,
      root,
      0,
    ],
    ...(packages.length > 0
      ? [
          [
            'linter',
            `Linter: ${packages.join(', ')}`,
            `cargo clippy -j 3 ${pkgArgs.join(' ')} --all-targets -- -D warnings`,
            root,
            10,
          ],
          [
            'testy',
            `Testy: ${packages.join(', ')}`,
            `cargo test --no-fail-fast -j 3 ${pkgArgs.join(' ')}`,
            root,
            10,
          ],
        ]
      : []),
    ['typy', 'Kontrola typów interfejsu', 'pnpm check', ui, 0],
    ['interfejs', 'Testy interfejsu', 'npx vitest run', ui, 0],
  ],
  wersja: [
    [
      'wersja',
      'Spójność numeru wersji (pliki, manifesty, Cargo.lock)',
      'node scripts/set-version.mjs --check',
      root,
      0,
    ],
    [
      'format',
      'Formatowanie Rust (workspace i powłoka)',
      `git ls-files '*.rs' | xargs -n 200 rustfmt --edition 2024 --check && cargo fmt --manifest-path ${SHELL} -- --check`,
      root,
      0,
    ],
    ['graf', 'Graf zależności modułów', 'bash scripts/check-deps.sh', root, 0],
    [
      'linter',
      'Linter całego projektu',
      'cargo clippy --locked -j 3 --workspace --all-targets -- -D warnings',
      root,
      30,
    ],
    [
      'linter-powloka',
      'Linter powłoki Tauri (zwykła i testowa)',
      `cargo clippy --locked -j 3 --manifest-path ${SHELL} --all-targets -- -D warnings && cargo clippy --locked -j 3 --manifest-path ${SHELL} --all-targets --features e2e -- -D warnings`,
      root,
      15,
    ],
    [
      'testy',
      'Testy całego projektu',
      'cargo test --locked --workspace --no-fail-fast -j 3',
      root,
      30,
    ],
    [
      'testy-powloka',
      'Testy powłoki Tauri (zwykła i testowa)',
      `cargo test --locked -j 3 --manifest-path ${SHELL} --lib && cargo test --locked -j 3 --manifest-path ${SHELL} --lib --features e2e`,
      root,
      15,
    ],
    [
      'schematy',
      'Schematy JSON zgodne z Rustem',
      'git diff --exit-code -- packages/schemas',
      root,
      0,
    ],
    [
      'tokeny',
      'Tokeny wyglądu i kontrast',
      'pnpm --filter @alfa/ui-kit build:tokens && git diff --exit-code -- packages/ui-kit/src/tokens.css packages/ui-kit/src/tokens.ts && pnpm --filter @alfa/ui-kit check:contrast',
      root,
      0,
    ],
    ['lint', 'Lint interfejsu i skryptów (eslint, Svelte 5, CSS)', 'pnpm lint', root, 0],
    [
      'typy-ipc',
      'Typy łącza zgodne ze schematem',
      'node scripts/gen-ipc-types.mjs --check',
      root,
      0,
    ],
    ['typy', 'Kontrola typów interfejsu', 'pnpm check', ui, 0],
    ['interfejs', 'Testy interfejsu', 'npx vitest run', ui, 0],
    [
      'przegladarka',
      'Budowa, budżet rozmiaru i testy w przeglądarce',
      `pnpm build && node scripts/bundle-size.mjs && PW_WORKERS=${e2eWorkers()} PW_CHANNEL=msedge pnpm test:e2e`,
      ui,
      0,
    ],
    [
      'aplikacja',
      'Budowa i test prawdziwej aplikacji (instancja testowa), pokrycie łącza',
      'bash scripts/verify-app.sh',
      root,
      15,
    ],
    [
      'dokumenty',
      'Plan w HTML: aktualność i odnośniki',
      'node scripts/docs-html.mjs --verify',
      root,
      0,
    ],
    ['zaleznosci', 'Zależności i licencje (cargo deny)', 'cargo deny check', root, 0],
    [
      'audyt-npm',
      'Podatności pakietów JS (pnpm audit)',
      'pnpm audit --audit-level moderate',
      root,
      0,
    ],
    [
      'sbom',
      'SBOM wydania (CycloneDX) i licencje interfejsu',
      'node scripts/sbom.mjs --check',
      root,
      0,
    ],
  ],
  sprzet: [
    ['nagrania', 'Nagrania kontrolne 16 kHz', 'node scripts/verify-audio.mjs', root, 0],
    ['sprzet', 'Testy sprzętowe', `node scripts/verify-hw.mjs ${flags.join(' ')}`, root, 15],
    [
      'proba-start',
      'Instancja testowa z kopią modelu',
      'bash scripts/trial/profile.sh start --model',
      root,
      10,
    ],
    [
      'obowiazki',
      'Próby obowiązków na lokalnym modelu z oceną',
      'node scripts/trial/duty.mjs',
      root,
      0,
    ],
    ['proba-stop', 'Zamknięcie instancji testowej', 'bash scripts/trial/profile.sh stop', root, 0],
  ],
}[level];

const freeGb = () => {
  const s = statfsSync(root);
  return Math.round((s.bavail * s.bsize) / 1024 ** 3);
};
/** Narzędzie wymagane przez etap (brak = etap NIEURUCHOMIONY, a cały bieg — niekompletny). */
const TOOLS = {
  zaleznosci: ['cargo', ['deny', '--version'], 'cargo-deny'],
  graf: ['jq', ['--version'], 'jq'],
};
const missingTool = (stage) => {
  const tool = TOOLS[stage];
  if (!tool) return null;
  return spawnSync(tool[0], tool[1], { encoding: 'utf8' }).status === 0 ? null : tool[2];
};

// Metryka biegu do rejestru dowodów (audyt A-15): kiedy, na czym, z którego commita.
const git = (...args) =>
  spawnSync('git', args, { cwd: root, encoding: 'utf8' }).stdout?.trim() ?? '';
const gpus = () => {
  if (process.platform !== 'win32') return [];
  const run = spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-Command',
      '(Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name) -join "|"',
    ],
    { encoding: 'utf8' },
  );
  return (run.stdout ?? '').trim().split('|').filter(Boolean);
};
const meta = {
  started: new Date().toISOString(),
  commit: git('rev-parse', 'HEAD'),
  dirty: git('status', '--porcelain', '--untracked-files=no') !== '',
  hardware: {
    cpu: cpus()[0]?.model?.trim() ?? '',
    ram_gb: Math.round(totalmem() / 1024 ** 3),
    gpus: gpus(),
  },
};

const results = [];
for (const [id, title, command, cwd, needGb] of STAGES) {
  const tool = missingTool(id);
  if (tool) {
    results.push({
      id,
      title,
      state: 'nieuruchomiony',
      why: `brak narzędzia ${tool} na tym komputerze`,
    });
    console.log(`— ${id}: NIEURUCHOMIONY (brak narzędzia)`);
    continue;
  }
  const free = freeGb();
  if (free < needGb) {
    results.push({
      id,
      title,
      state: 'nieuruchomiony',
      why: `za mało miejsca na dysku: ${free} GB, potrzeba ${needGb} GB`,
    });
    console.log(`— ${id}: NIEURUCHOMIONY (dysk ${free} GB)`);
    continue;
  }
  const started = Date.now();
  const run = spawnSync(BASH, ['-c', command], {
    cwd,
    env: stageEnv(),
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  const output = `${run.stdout ?? ''}\n${run.stderr ?? ''}`.replace(ANSI, '');
  writeFileSync(join(out, `${level}-${id}.log`), output);
  const seconds = Math.round((Date.now() - started) / 1000);
  const state = run.status === 0 ? 'zielony' : 'CZERWONY';
  results.push({
    id,
    title,
    state,
    seconds,
    numbers: numbersFrom(id, output),
    command,
    log: `${level}-${id}.log`,
  });
  console.log(`— ${id}: ${state} (${seconds} s) ${numbersFrom(id, output)}`);
}

/** Liczby do raportu wyjęte z wyjścia etapu (nie z pamięci). */
function numbersFrom(id, output) {
  if (id === 'testy') {
    const rows = [
      ...output.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/g),
    ];
    const sum = (i) => rows.reduce((total, m) => total + Number(m[i]), 0);
    return `${rows.length} programów testowych, ${sum(1)} zielonych, ${sum(2)} czerwonych, ${sum(3)} pominiętych`;
  }
  if (id === 'interfejs') return output.match(/Tests\s+(.+)/)?.[1]?.trim() ?? '';
  if (id === 'przegladarka') return output.match(/^\s*(\d+ passed.*)$/m)?.[1]?.trim() ?? '';
  if (id === 'aplikacja') {
    const passed = output.match(/^\s*(\d+ passed.*)$/m)?.[1]?.trim() ?? '';
    const coverage = output.match(/przećwiczone w prawdziwej aplikacji: (.+)/)?.[1] ?? '';
    return [passed, coverage && `pokrycie łącza ${coverage}`].filter(Boolean).join('; ');
  }
  if (id === 'sprzet' || id === 'obowiazki') {
    return output
      .split(/\r?\n/)
      .filter((line) => line.startsWith('— '))
      .map((line) => line.slice(2).trim())
      .join(' · ');
  }
  return '';
}

const count = (state) => results.filter((r) => r.state === state).length;
const other = { szybki: ['wersja', 'sprzet'], wersja: ['sprzet'], sprzet: [] }[level];
// Audyt A-09: bieg z pominiętym etapem nie jest „zielony” — jest NIEKOMPLETNY (kod 3).
const verdict =
  count('CZERWONY') > 0 ? 'CZERWONY' : count('nieuruchomiony') > 0 ? 'NIEKOMPLETNY' : 'ZIELONY';
const lines = [
  `# Sprawdzenie „${level}” — wersja ${version}: ${verdict}`,
  '',
  `Bieg: ${new Date().toISOString()} · commit ${spawnSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim()} · zielone ${count('zielony')}, czerwone ${count('CZERWONY')}, nieuruchomione ${count('nieuruchomiony')}`,
  '',
  '| Etap | Wynik | Czas | Liczby / powód |',
  '| --- | --- | --- | --- |',
  ...results.map(
    (r) =>
      `| ${r.title} | ${r.state} | ${r.seconds === undefined ? '—' : `${r.seconds} s`} | ${r.why ?? r.numbers ?? ''} |`,
  ),
  '',
  '## Czego ten bieg nie obejmuje',
  '',
  ...other.map((name) => `- poziom „${name}” — osobny bieg \`node scripts/verify.mjs ${name}\``),
  ...(level === 'sprzet' && !flags.includes('--pulpit')
    ? [
        '- testy dotykające pulpitu (schowek, Kosz, skróty, Notatnik, Edge, poświadczenia) — bieg z `--pulpit`, gdy właściciel nie pracuje',
      ]
    : []),
  ...(level === 'sprzet' && !flags.includes('--dzwiek')
    ? ['- etapy odtwarzające dźwięk — bieg z `--dzwiek`, tylko w dzień']
    : []),
  '- kamera (odłączona sprzętowo), ocena brzmienia głosów słuchem, decyzje bezpieczeństwa — wymagają właściciela',
  '',
];
const hw = join(root, 'target', 'verify', 'hw', 'raport.md');
if (level === 'sprzet' && existsSync(hw)) lines.push(readFileSync(hw, 'utf8'));
writeFileSync(join(out, `raport-${level}.md`), `${lines.join('\n')}\n`);
writeFileSync(
  join(out, `raport-${level}.json`),
  `${JSON.stringify(
    { version, level, verdict, ...meta, finished: new Date().toISOString(), results },
    null,
    2,
  )}\n`,
);
console.log(`Raport: ${join(out, `raport-${level}.md`)}`);
console.log(`Wynik: ${verdict}`);
process.exit({ CZERWONY: 1, NIEKOMPLETNY: 3, ZIELONY: 0 }[verdict]);
