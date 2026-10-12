#!/usr/bin/env node
// Testy sprzętowe na maszynie użytkownika (docs/plan/24 §2.2): uruchamia testy `#[ignore]`
// i testy „na żywo” pakiet po pakiecie, ze zmiennymi wskazującymi modele i silniki
// z `%LOCALAPPDATA%\Alfa` (tylko odczyt). Wynik: `target/verify/hw/raport.md` i `raport.json`.
//
//   node scripts/verify-hw.mjs                 etapy „ciche” (bez śladu na pulpicie, bez dźwięku)
//   node scripts/verify-hw.mjs --pulpit        także schowek, Kosz, skróty, Notatnik, Edge, poświadczenia
//   node scripts/verify-hw.mjs --dzwiek        także etapy, które odtwarzają dźwięk
//   node scripts/verify-hw.mjs --only vad,stt  wybrane etapy;  --list  spis etapów
//
// Etap bez wymaganych plików jest POMIJANY Z POWODEM (raport go wymienia) — nigdy zaliczany.
// Przed startem: `source scripts/portable-env.sh` (cargo, Perl do OpenSSL).
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const only = args.includes('--only') ? args[args.indexOf('--only') + 1].split(',') : null;

const local = process.env.LOCALAPPDATA ?? '';
const alfa = join(local, 'Alfa');
const models = join(alfa, 'models');
const sidecars = join(alfa, 'sidecars');
const audio = join(root, 'target', 'verify', 'audio');
const wav = (n) => join(audio, `mowa-0${n}-16k.wav`);
const firstFile = (dir, pattern) => {
  if (!existsSync(dir)) return null;
  const hit = readdirSync(dir).find((name) => pattern.test(name));
  return hit ? join(dir, hit) : null;
};
const llamaServer =
  firstFile(join(sidecars, 'llama-cuda'), /^llama-server\.exe$/) ??
  firstFile(join(sidecars, 'llama-cpu'), /^llama-server\.exe$/);

/**
 * Etapy. `group`: `cichy` (zawsze), `pulpit` (dotyka pulpitu właściciela), `dzwiek` (odtwarza dźwięk).
 * `needs`: pliki, bez których etap jest pomijany z powodem. `env`: zmienne dla testu.
 */
const STAGES = [
  {
    id: 'vad',
    title: 'Silero VAD na prawdziwym modelu i nagraniu mowy',
    group: 'cichy',
    pkg: 'voice-vad-impl',
    test: ['--test', 'vad', '--', '--ignored', 'silero_real_model'],
    needs: [join(models, 'silero', 'silero_vad.onnx'), wav(1)],
    env: { ALFA_SILERO_VAD: join(models, 'silero', 'silero_vad.onnx'), ALFA_SPEECH_WAV: wav(1) },
  },
  {
    id: 'turn',
    title: 'Smart Turn — proces poboczny na prawdziwym modelu',
    group: 'cichy',
    pkg: 'voice-turn-impl',
    test: ['--test', 'smart_turn_live', '--', '--nocapture'],
    needs: [
      join(sidecars, 'smart-turn', 'alfa-smart-turn.exe'),
      join(models, 'turn', 'smart-turn-v3.2-cpu.onnx'),
      join(sidecars, 'onnxruntime', 'onnxruntime.dll'),
      wav(1),
    ],
    env: {
      ALFA_SMART_TURN_EXE: join(sidecars, 'smart-turn', 'alfa-smart-turn.exe'),
      ALFA_SMART_TURN_MODEL: join(models, 'turn', 'smart-turn-v3.2-cpu.onnx'),
      ALFA_SMART_TURN_ORT: join(sidecars, 'onnxruntime', 'onnxruntime.dll'),
      ALFA_SMART_TURN_WAV: wav(1),
    },
  },
  {
    id: 'stt',
    title: 'Qwen3-ASR — rozpoznanie nagrania na karcie graficznej',
    group: 'cichy',
    pkg: 'voice-stt-impl',
    test: ['--test', 'qwen_live', '--', '--nocapture'],
    needs: [
      llamaServer,
      join(models, 'qwen3-asr', 'Qwen3-ASR-1.7B-Q8_0.gguf'),
      join(models, 'qwen3-asr', 'mmproj-Qwen3-ASR-1.7B-Q8_0.gguf'),
      wav(1),
    ],
    env: {
      ALFA_QWEN_SERVER: llamaServer ?? '',
      ALFA_QWEN_MODEL: join(models, 'qwen3-asr', 'Qwen3-ASR-1.7B-Q8_0.gguf'),
      ALFA_QWEN_MMPROJ: join(models, 'qwen3-asr', 'mmproj-Qwen3-ASR-1.7B-Q8_0.gguf'),
      ALFA_QWEN_WAV: wav(1),
    },
  },
  {
    id: 'rozmowa',
    title: 'Rozmowa „na sucho”: nagranie zamiast mikrofonu, prawdziwe silniki, próba silników',
    group: 'cichy',
    pkg: 'app-voice',
    test: ['--test', 'live_talk', '--', '--nocapture'],
    needs: [wav(1)],
    env: { ALFA_LIVE_TALK: '1', ALFA_LIVE_TALK_WAV: wav(1) },
  },
  {
    id: 'mikrofon',
    title: 'Potok głosu na prawdziwym mikrofonie (po cichu)',
    group: 'cichy',
    pkg: 'app-core',
    test: ['--test', 'live_mic', '--', '--nocapture'],
    needs: [],
    env: { ALFA_LIVE_MIC: '1' },
  },
  {
    id: 'mowca',
    title: 'Model mówcy: rejestracja trzema nagraniami, weryfikacja czwartym',
    group: 'cichy',
    pkg: 'voice-speaker-impl',
    test: ['--test', 'speaker', '--', '--ignored', '--nocapture', 'real_model'],
    needs: [
      join(models, 'speaker', 'wespeaker-resnet34.speaker.json'),
      wav(1),
      wav(2),
      wav(3),
      wav(4),
    ],
    env: {
      ALFA_SPEAKER_MODEL: join(models, 'speaker', 'wespeaker-resnet34.speaker.json'),
      ALFA_SPEAKER_WAVS: [wav(1), wav(2), wav(3), wav(4)].join(','),
    },
  },
  {
    id: 'embed',
    title: 'Embeddingi: prawdziwy model, sens zdań',
    group: 'cichy',
    pkg: 'lib-embed',
    test: ['--test', 'it', '--', '--ignored', '--nocapture', 'real_model_semantic_sanity'],
    needs: [join(models, 'embed', 'multilingual-e5-small', 'embed.json')],
    env: { ALFA_EMBED_MODEL: join(models, 'embed', 'multilingual-e5-small', 'embed.json') },
  },
  {
    id: 'slowo',
    title: 'Słowo wywoławcze na prawdziwym modelu',
    group: 'cichy',
    pkg: 'voice-wake-impl',
    test: ['--test', 'kws_onnx', '--', '--ignored', '--nocapture', 'real_model'],
    needs: [firstFile(join(models, 'kws'), /\.kws\.json$/)],
    env: {
      ALFA_KWS_MODEL: firstFile(join(models, 'kws'), /\.kws\.json$/) ?? '',
      ALFA_KWS_WAV: wav(1),
    },
  },
  {
    id: 'audio',
    title: 'Urządzenia audio: zestaw kontraktowy na sprzęcie',
    group: 'dzwiek',
    pkg: 'voice-audio-impl',
    test: ['--test', 'module', '--', '--ignored', '--nocapture', 'contract_suite_on_hardware'],
    needs: [],
    env: {},
  },
  {
    id: 'system',
    title: 'System: procesy, usługi, zdarzenia, zmienne środowiska',
    group: 'cichy',
    pkg: 'platform-windows-sys-impl',
    test: [
      '--test',
      'live_sys',
      '--',
      '--ignored',
      '--nocapture',
      'processes_and_own',
      'services_events',
    ],
    needs: [],
    env: {},
  },
  {
    id: 'rejestr',
    title: 'Rejestr: odczyt i odmowa odczytu sekretów',
    group: 'cichy',
    pkg: 'platform-windows-office-impl',
    test: [
      '--test',
      'live_windows',
      '--',
      '--ignored',
      '--nocapture',
      'registry_reads_and_denies_secrets',
    ],
    needs: [],
    env: {},
  },
  {
    id: 'markdown',
    title: 'Markdown: 100 kB w budżecie czasu (budowa wydaniowa)',
    group: 'cichy',
    pkg: 'lib-markdown',
    release: true,
    test: ['--test', 'perf', '--', '--ignored', '--nocapture'],
    needs: [],
    env: {},
  },
  {
    id: 'pobranie-motw',
    title: 'Pobrany plik dostaje znacznik pochodzenia z Internetu',
    group: 'cichy',
    pkg: 'platform-windows-sys-impl',
    test: ['--test', 'live_sys', '--', '--ignored', '--nocapture', 'download_gets_mark_of_the_web'],
    needs: [],
    env: {},
  },
  {
    id: 'schowek',
    title: 'Schowek, Kosz i skróty globalne na pulpicie',
    group: 'pulpit',
    pkg: 'platform-windows-impl',
    test: ['--test', 'windows_system', '--', '--ignored', '--nocapture', '--test-threads', '1'],
    needs: [],
    env: {},
  },
  {
    id: 'pulpit',
    title: 'Pulpit: Notatnik przez UI Automation, zrzut z maskami, ochrona okien Alfy',
    group: 'pulpit',
    pkg: 'platform-windows-gui-impl',
    test: ['--test', 'gui_windows', '--', '--ignored', '--nocapture', '--test-threads', '1'],
    needs: [],
    env: {},
  },
  {
    id: 'wejscie',
    title: 'Wejście wysyłane do okna Brokera jest zawsze oznaczone',
    group: 'pulpit',
    pkg: 'platform-windows-kernel-impl',
    test: ['--test', 'kernel_windows', '--', '--ignored', '--nocapture'],
    needs: [],
    env: {},
  },
  {
    id: 'edge',
    title: 'Edge sterowany potokiem z filtrem ruchu wychodzącego',
    group: 'pulpit',
    pkg: 'platform-windows-office-impl',
    test: ['--test', 'live_windows', '--', '--ignored', '--nocapture', 'edge_over_pipe'],
    needs: [],
    env: {},
  },
  {
    id: 'poswiadczenia',
    title: 'Menedżer poświadczeń Windows: zapis, odczyt, lista, usunięcie wpisu testowego',
    group: 'pulpit',
    pkg: 'accounts-hub-impl',
    test: ['--test', 'credman', '--', '--ignored', '--nocapture'],
    needs: [],
    env: {},
  },
];

/** Testy świadomie poza biegiem — raport wymienia je z powodem. */
const EXCLUDED = [
  [
    'kamera (`platform-windows-impl::camera_hw`)',
    'kamera laptopa odłączona sprzętowo — potrzebny właściciel',
  ],
  ['blokada ekranu i gra na pełnym ekranie (`sys_windows`)', 'wymaga ręcznej akcji w ciągu 30 s'],
  ['Word (`live_windows::word_reads_and_edits_copy`)', 'wymaga Worda i pliku ALFA_LIVE_DOCX'],
  [
    'Broker i Jądro: P3-02, P3-04, W3-09, PT-25',
    'czerwone celowo — czekają na decyzję właściciela',
  ],
  ['usługa LocalSystem (`app-safety::windows_ports`)', 'wymaga uprawnień administratora'],
  ['korpus mowy F2 (`f2_eval::real_models_on_corpus`)', 'brak korpusu nagrań na tym komputerze'],
  ['katalog na żywo (`app-models::live_catalog`)', 'pobiera ok. 4 GB — osobny bieg na polecenie'],
];

if (flag('--list')) {
  for (const s of STAGES) console.log(`${s.id.padEnd(16)} ${s.group.padEnd(7)} ${s.title}`);
  process.exit(0);
}

const wanted = (s) => {
  if (only) return only.includes(s.id);
  if (s.group === 'pulpit') return flag('--pulpit');
  if (s.group === 'dzwiek') return flag('--dzwiek');
  return true;
};

const results = [];
for (const stage of STAGES) {
  if (!wanted(stage)) {
    const why = only
      ? 'poza wyborem --only'
      : `grupa „${stage.group}” — uruchamiana z --${stage.group}`;
    results.push({ ...brief(stage), state: 'pominięty', why });
    continue;
  }
  const missing = stage.needs.filter((path) => !path || !existsSync(path));
  if (missing.length > 0) {
    const names = missing.map((p) => p ?? '(plik, którego wzorzec nic nie znalazł)').join(', ');
    results.push({ ...brief(stage), state: 'pominięty', why: `brak: ${names}` });
    console.log(`— ${stage.id}: POMINIĘTY (brak plików)`);
    continue;
  }
  const cargo = [
    'test',
    '-j',
    '3',
    ...(stage.release ? ['--release'] : []),
    '-p',
    stage.pkg,
    ...stage.test,
  ];
  const started = Date.now();
  const run = spawnSync('cargo', cargo, {
    cwd: root,
    env: { ...process.env, ...stage.env },
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    timeout: 30 * 60_000,
  });
  const output = `${run.stdout ?? ''}\n${run.stderr ?? ''}`;
  const totals = [
    ...output.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/g),
  ];
  const passed = totals.reduce((sum, m) => sum + Number(m[1]), 0);
  const failed = totals.reduce((sum, m) => sum + Number(m[2]), 0);
  const seconds = Math.round((Date.now() - started) / 1000);
  // Zero wykonanych testów to nie sukces: filtr nazwy nie trafił albo test sam się pominął.
  const skippedInside = /pomijam|nie ?ustawione/i.test(output);
  const state =
    run.status !== 0 || failed > 0
      ? 'CZERWONY'
      : passed === 0 || skippedInside
        ? 'pominięty'
        : 'zielony';
  const tail = output.trim().split(/\r?\n/).slice(-25).join('\n');
  results.push({
    ...brief(stage),
    state,
    passed,
    failed,
    seconds,
    why:
      state === 'pominięty'
        ? passed === 0
          ? 'żaden test się nie wykonał (filtr albo warunek w teście)'
          : 'test sam zgłosił pominięcie — sprawdź zmienne'
        : undefined,
    tail: state === 'zielony' ? undefined : tail,
    notes: output
      .split(/\r?\n/)
      .filter((line) => /^(POMIAR|WYNIK|pomiar|wynik)\b/.test(line.trim())),
  });
  console.log(`— ${stage.id}: ${state} (${passed} zal., ${failed} obl., ${seconds} s)`);
}

function brief(stage) {
  return { id: stage.id, title: stage.title, group: stage.group, pkg: stage.pkg };
}

const outDir = join(root, 'target', 'verify', 'hw');
mkdirSync(outDir, { recursive: true });
const count = (state) => results.filter((r) => r.state === state).length;
const lines = [
  '# Testy sprzętowe na maszynie użytkownika',
  '',
  `Bieg: ${new Date().toISOString()} · zielone ${count('zielony')}, czerwone ${count('CZERWONY')}, pominięte ${count('pominięty')}`,
  '',
  '| Etap | Co sprawdza | Wynik | Czas | Uwagi |',
  '| --- | --- | --- | --- | --- |',
  ...results.map((r) => {
    const tests = r.passed === undefined ? '' : ` (${r.passed} zal., ${r.failed} obl.)`;
    const time = r.seconds === undefined ? '—' : `${r.seconds} s`;
    return `| ${r.id} | ${r.title} | ${r.state}${tests} | ${time} | ${r.why ?? ''} |`;
  }),
  '',
  '## Poza biegiem',
  '',
  '| Test | Powód |',
  '| --- | --- |',
  ...EXCLUDED.map(([name, why]) => `| ${name} | ${why} |`),
  '',
];
for (const r of results.filter((x) => x.tail)) {
  lines.push(`## ${r.id} — koniec wyjścia`, '', '```', r.tail, '```', '');
}
writeFileSync(join(outDir, 'raport.md'), `${lines.join('\n')}\n`);
writeFileSync(
  join(outDir, 'raport.json'),
  `${JSON.stringify({ results, excluded: EXCLUDED }, null, 2)}\n`,
);
console.log(`Raport: ${join(outDir, 'raport.md')}`);
process.exit(count('CZERWONY') > 0 ? 1 : 0);
