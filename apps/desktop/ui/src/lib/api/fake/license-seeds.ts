// Atrapa: karty licencji pozycji katalogu (C5.7) — skrót danych produkcyjnych
// `crates/app-models/data/license-cards.json` (te same identyfikatory, licencje, użycie komercyjne
// i sprawy otwarte; uwagi krótsze). Głosy: barwy Supertonic 3, Gosia (Piper), mówczynie Pocket TTS.
import type { LocalizedText } from '../types';
import type { CommercialUse, LicenseCard, VoiceLicense } from '../types-models';

const L = (pl: string, en: string): LocalizedText => ({ pl, en });
const CONFIRM = L(' Do potwierdzenia przez człowieka.', ' To be confirmed by a human.');
const WATERMARK = L(
  ' Znak wodny w mowie syntetycznej: decyzja właściciela w toku (C5.7) — Alfa go dziś nie dodaje.',
  ' Synthetic-speech watermark: owner decision pending (C5.7) — Alfa does not add one today.',
);
const APACHE = 'https://www.apache.org/licenses/LICENSE-2.0';
const MIT_ORG = 'https://opensource.org/license/mit';
const CUDA = 'https://docs.nvidia.com/cuda/eula/index.html';
const LLAMA = 'https://github.com/ggml-org/llama.cpp/blob/master/LICENSE';
const WHISPER = 'https://github.com/openai/whisper/blob/main/LICENSE';
const ORT = 'https://github.com/microsoft/onnxruntime/blob/main/LICENSE';

type Row = [
  id: string,
  name: string,
  url: string,
  author: LocalizedText,
  commercial: CommercialUse,
  confirmed: boolean,
  notes: LocalizedText,
  extra?: { speech?: boolean; voices?: readonly VoiceLicense[] },
];

const PERMISSIVE = L(
  'Wolno używać i rozpowszechniać, także komercyjnie, z kopią licencji.',
  'May be used and redistributed, also commercially, with a copy of the licence.',
);
const NVIDIA = L(
  'Serwer na MIT; biblioteki cudart i cuBLAS — umowa licencyjna NVIDIA (rozpowszechnianie pod jej warunkami).',
  'Server is MIT; cudart and cuBLAS — NVIDIA licence agreement (shipping under its terms).',
);
const GGML = L('ggml-org (llama.cpp, whisper.cpp)', 'ggml-org (llama.cpp, whisper.cpp)');
const SPEAKLEASH = L(
  'SpeakLeash i ACK Cyfronet AGH, Polska',
  'SpeakLeash and ACK Cyfronet AGH, Poland',
);
const OPENAI = L(
  'OpenAI (wagi); ggml-org (zapis GGML)',
  'OpenAI (weights); ggml-org (GGML conversion)',
);

const SUPERTONIC_NAMES: readonly (readonly [string, string, string])[] = [
  ['F1', 'kobiecy, ciepły (domyślnie: Alfa)', 'female, warm (default: Alfa)'],
  ['F2', 'kobiecy (domyślnie: Beta)', 'female (default: Beta)'],
  ['F3', 'kobiecy', 'female'],
  ['F4', 'kobiecy (domyślnie: Gama)', 'female (default: Gama)'],
  ['F5', 'kobiecy, wyrazisty (domyślnie: Delta)', 'female, expressive (default: Delta)'],
  ['M1', 'męski', 'male'],
  ['M2', 'męski', 'male'],
  ['M3', 'męski', 'male'],
  ['M4', 'męski', 'male'],
  ['M5', 'męski', 'male'],
];

const voice = (
  id: string,
  name: LocalizedText,
  license: string,
  commercial: CommercialUse,
  origin: LocalizedText,
  notes: LocalizedText,
): VoiceLicense => ({ id, name, license, origin, commercial, notes });

const SUPERTONIC_VOICES = SUPERTONIC_NAMES.map(([id, pl, en]) =>
  voice(
    id,
    L(`${id} — ${pl}`, `${id} — ${en}`),
    'OpenRAIL-M',
    'conditional',
    L('Barwa wbudowana w model Supertonic 3', 'Timbre built into the Supertonic 3 model'),
    L(
      'Warunki jak wagi modelu; nie wolno jej używać do podszywania się pod prawdziwą osobę. Pochodzenie nagrań wzorcowych — do potwierdzenia przez człowieka (C5.6).',
      'Same terms as the model weights; must not be used to impersonate a real person. Origin of the reference recordings — to be confirmed by a human (C5.6).',
    ),
  ),
);

const ROWS: readonly Row[] = [
  [
    'bielik-4.5b-v3.0-instruct-q8_0',
    'Apache License 2.0',
    APACHE,
    SPEAKLEASH,
    'conditional',
    false,
    L(
      'Apache-2.0 według autorów; model bazowy Qwen2.5-3B ma licencję badawczą Alibaba — czy jej warunki przechodzą na Bielika:',
      'Apache-2.0 per the authors; the Qwen2.5-3B base model has an Alibaba research licence — whether its terms carry over:',
    ),
  ],
  [
    'bielik-1.5b-v3.0-instruct-q8_0',
    'Apache License 2.0',
    APACHE,
    SPEAKLEASH,
    'yes',
    false,
    PERMISSIVE,
  ],
  [
    'multilingual-e5-small',
    'MIT License',
    MIT_ORG,
    L('Microsoft (intfloat)', 'Microsoft (intfloat)'),
    'yes',
    false,
    PERMISSIVE,
  ],
  ['whisper-large-v3-turbo-q5_0', 'MIT License', WHISPER, OPENAI, 'yes', false, PERMISSIVE],
  ['whisper-small-q5_1', 'MIT License', WHISPER, OPENAI, 'yes', false, PERMISSIVE],
  [
    'qwen3-asr-1.7b-q8',
    'Apache License 2.0',
    APACHE,
    L('Zespół Qwen (Alibaba Cloud)', 'Qwen team (Alibaba Cloud)'),
    'yes',
    true,
    L(
      'Wolno używać, zmieniać i rozpowszechniać, także komercyjnie. Uruchamia go llama-server (osobna karta).',
      'May be used, modified and redistributed, also commercially. Runs on llama-server (separate card).',
    ),
  ],
  [
    'supertonic-3',
    'OpenRAIL-M (wagi modelu i barwy głosów); MIT (kod przykładowy)',
    'https://huggingface.co/supertone-oss-archive/supertonic-3',
    L('Supertone Inc. (Korea Południowa)', 'Supertone Inc. (South Korea)'),
    'conditional',
    true,
    L(
      'OpenRAIL-M: użycie komercyjne z kopią licencji i listą zakazanych zastosowań (m.in. podszywanie się pod osoby, treści bez oznaczenia AI, udzielanie porad medycznych — to ostatnie czeka na decyzję właściciela).',
      'OpenRAIL-M: commercial use with a copy of the licence and the list of prohibited uses (impersonation, content not disclosed as AI, giving medical advice — the last awaits the owner).',
    ),
    { speech: true, voices: SUPERTONIC_VOICES },
  ],
  [
    'piper-pl_PL-gosia-medium',
    'MIT (repozytorium piper-voices); licencja nagrań — według MODEL_CARD',
    'https://huggingface.co/rhasspy/piper-voices/blob/v1.0.0/pl/pl_PL/gosia/medium/MODEL_CARD',
    L('Projekt Piper (rhasspy/piper-voices)', 'Piper project (rhasspy/piper-voices)'),
    'unknown',
    false,
    L(
      'Licencja zbioru nagrań głosu „gosia” — według karty głosu.',
      'Licence of the “gosia” recording set — per the voice card.',
    ),
    {
      speech: true,
      voices: [
        voice(
          'pl_PL-gosia-medium',
          L(
            'Gosia (pl_PL, medium) — głos zapasowy agentek',
            'Gosia (pl_PL, medium) — the agents’ fallback voice',
          ),
          'do potwierdzenia — MODEL_CARD',
          'unknown',
          L('Model głosu Pipera', 'Piper voice model'),
          L(
            'Licencja nagrań — do potwierdzenia przez człowieka.',
            'Recording licence — to be confirmed by a human.',
          ),
        ),
      ],
    },
  ],
  [
    'silero-vad',
    'MIT License',
    'https://github.com/snakers4/silero-vad/blob/master/LICENSE',
    L('Silero Team (snakers4)', 'Silero Team (snakers4)'),
    'yes',
    true,
    PERMISSIVE,
  ],
  [
    'smart-turn-v3.2',
    'BSD 2-Clause License',
    'https://opensource.org/license/bsd-2-clause',
    L('Daily — projekt Pipecat', 'Daily — Pipecat project'),
    'yes',
    true,
    PERMISSIVE,
  ],
  [
    'openwakeword-features',
    'Apache License 2.0 (kod); licencja modeli cech — do potwierdzenia',
    'https://github.com/dscripka/openWakeWord/blob/main/LICENSE',
    L('David Scripka (openWakeWord)', 'David Scripka (openWakeWord)'),
    'unknown',
    false,
    L(
      'Według README wytrenowane modele są na CC BY-NC-SA 4.0 — czy dotyczy to modeli cech:',
      'Per the README the pre-trained models are CC BY-NC-SA 4.0 — whether this covers the feature models:',
    ),
  ],
  [
    'wespeaker-resnet34',
    'Apache License 2.0 (WeSpeaker); warunki zbioru VoxCeleb — do sprawdzenia',
    'https://github.com/wenet-e2e/wespeaker/blob/master/LICENSE',
    L('Projekt WeSpeaker (WeNet)', 'WeSpeaker project (WeNet)'),
    'unknown',
    false,
    L(
      'Czy warunki zbioru VoxCeleb ograniczają użycie komercyjne:',
      'Whether the VoxCeleb terms limit commercial use:',
    ),
  ],
  ['sidecar-llama-vulkan', 'MIT License', LLAMA, GGML, 'yes', true, PERMISSIVE],
  [
    'sidecar-llama-cuda',
    'MIT (llama.cpp); NVIDIA CUDA Toolkit EULA (cudart, cuBLAS)',
    CUDA,
    GGML,
    'conditional',
    false,
    NVIDIA,
  ],
  ['sidecar-llama-cpu', 'MIT License', LLAMA, GGML, 'yes', true, PERMISSIVE],
  [
    'sidecar-whisper-cpu',
    'MIT License',
    'https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE',
    GGML,
    'yes',
    false,
    PERMISSIVE,
  ],
  [
    'sidecar-whisper-cuda',
    'MIT (whisper.cpp); NVIDIA CUDA Toolkit EULA (cudart, cuBLAS)',
    CUDA,
    GGML,
    'conditional',
    false,
    NVIDIA,
  ],
  ['onnxruntime-cpu', 'MIT License', ORT, L('Microsoft', 'Microsoft'), 'yes', true, PERMISSIVE],
  [
    'sidecar-piper',
    'MIT (Piper 2023.11.14-2); GPL-3.0 (espeak-ng w archiwum)',
    'https://github.com/rhasspy/piper/blob/master/LICENSE.md',
    L('Michael Hansen (rhasspy/piper)', 'Michael Hansen (rhasspy/piper)'),
    'conditional',
    false,
    L(
      'Archiwum zawiera espeak-ng na GPL-3.0 — rozpowszechnianie pod warunkami GPL.',
      'The archive contains espeak-ng under GPL-3.0 — shipping under GPL terms.',
    ),
    { speech: true },
  ],
  [
    'sidecar-pocket-tts',
    'CC BY 4.0 (model Pocket TTS); wrapper — kod Alfy',
    'https://creativecommons.org/licenses/by/4.0/',
    L(
      'Kyutai (model bazowy); wariant polski — do wyjaśnienia (C5.1)',
      'Kyutai (base model); Polish variant — to be clarified (C5.1)',
    ),
    'unknown',
    false,
    L(
      'Skąd pochodzi model „Pocket TTS PL” i na jakiej licencji:',
      'Where the “Pocket TTS PL” model comes from and under which licence:',
    ),
    {
      speech: true,
      voices: ['pl-f1', 'pl-f2'].map((id) =>
        voice(
          id,
          L(`${id} — mówczyni bazowa`, `${id} — base speaker`),
          'CC-BY-4.0 (do potwierdzenia)',
          'unknown',
          L(
            'Głos modelu społeczności „Pocket TTS PL”',
            'Voice of the community “Pocket TTS PL” model',
          ),
          L(
            'Licencja i pochodzenie — do potwierdzenia przez człowieka.',
            'Licence and origin — to be confirmed by a human.',
          ),
        ),
      ),
    },
  ],
];

/** Identyfikator SPDX po początku nazwy licencji (reszta — pierwszy człon nazwy). */
const SPDX: Readonly<Record<string, string>> = {
  'Apache License 2.0': 'Apache-2.0',
  'MIT License': 'MIT',
  'BSD 2-Clause License': 'BSD-2-Clause',
};

function card([, name, url, author, commercial, confirmed, notes, extra]: Row): LicenseCard {
  const tail = (lang: 'pl' | 'en') =>
    notes[lang] + (confirmed ? '' : CONFIRM[lang]) + (extra?.speech ? WATERMARK[lang] : '');
  return {
    id:
      Object.entries(SPDX).find(([prefix]) => name.startsWith(prefix))?.[1] ??
      name.split(' ')[0] ??
      name,
    name,
    url,
    author,
    commercial,
    attribution: true,
    notes: L(tail('pl'), tail('en')),
    confirmed,
    voices: extra?.voices ?? [],
  };
}

const CARDS = new Map(ROWS.map((row) => [row[0], card(row)]));

/** Karta licencji pozycji atrapy (`null` — pozycja spoza katalogu kart). */
export function seedLicense(id: string): LicenseCard | null {
  return CARDS.get(id) ?? null;
}
