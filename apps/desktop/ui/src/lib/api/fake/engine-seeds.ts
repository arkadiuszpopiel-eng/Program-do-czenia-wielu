// Atrapa: pozycje katalogu „Modele i silniki” (te same identyfikatory, rodzaje i możliwość
// pobrania co katalog produkcyjny `app_models::builtin` — pakiety 1–6 składają się z nich).
import type { ModelItem, ModelItemKind } from '../types-models';

const MIB = 1024 * 1024;

/** Deterministyczny „SHA-256” atrapy (64 znaki hex z identyfikatora pliku). */
export function fakeSha(seed: string): string {
  let h = 2166136261;
  let out = '';
  while (out.length < 64) {
    for (const c of seed + out.length) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
    out += h.toString(16).padStart(8, '0');
  }
  return out.slice(0, 64);
}

export type Seed = [
  id: string,
  kind: ModelItemKind,
  name: string,
  license: string,
  mib: number,
  files: readonly string[],
  pinned: boolean,
  confirmed: boolean,
  note: readonly [string, string],
];

export const SEEDS: readonly Seed[] = [
  [
    'bielik-4.5b-v3.0-instruct-q8_0',
    'llm',
    'Bielik 4.5B v3.0 Instruct (Q8_0)',
    'Apache-2.0',
    4826,
    ['Bielik-4.5B-v3.0-Instruct.Q8_0.gguf'],
    false,
    false,
    [
      'Lokalny model rozmowy (llama.cpp). Wymaga sidecara llama-server.',
      'Local chat model (llama.cpp). Requires the llama-server sidecar.',
    ],
  ],
  [
    'bielik-1.5b-v3.0-instruct-q8_0',
    'llm',
    'Bielik 1.5B v3.0 Instruct (Q8_0)',
    'Apache-2.0',
    1620,
    ['Bielik-1.5B-v3.0-Instruct.Q8_0.gguf'],
    false,
    false,
    [
      'Lżejszy lokalny model rozmowy (bez narzędzi agentek). Wymaga sidecara llama-server.',
      'Lighter local chat model (no agent tools). Requires the llama-server sidecar.',
    ],
  ],
  [
    'multilingual-e5-small',
    'embed',
    'Multilingual E5 small (ONNX fp32)',
    'MIT',
    488,
    ['onnx/model.onnx', 'tokenizer.json'],
    false,
    false,
    ['Embedder wyszukiwania semantycznego (pamięć F7).', 'Semantic search embedder (F7 memory).'],
  ],
  [
    'whisper-large-v3-turbo-q5_0',
    'stt',
    'Whisper large-v3-turbo-q5_0',
    'MIT',
    547,
    ['ggml-large-v3-turbo-q5_0.bin'],
    false,
    false,
    [
      'Rozpoznawanie mowy. Wymaga sidecara whisper-server.',
      'Speech recognition. Requires the whisper-server sidecar.',
    ],
  ],
  [
    'whisper-small-q5_1',
    'stt',
    'Whisper small-q5_1',
    'MIT',
    181,
    ['ggml-small-q5_1.bin'],
    false,
    false,
    ['Lżejszy model rozpoznawania mowy na CPU (zapas).', 'Lighter CPU speech model (fallback).'],
  ],
  [
    'piper-pl_PL-gosia-medium',
    'tts',
    'Piper pl_PL gosia (medium)',
    'MIT (głos: do potwierdzenia — MODEL_CARD)',
    61,
    ['pl_PL-gosia-medium.onnx', 'pl_PL-gosia-medium.onnx.json'],
    false,
    false,
    ['Głos TTS Piper. Wymaga sidecara piper.', 'Piper TTS voice. Requires the piper sidecar.'],
  ],
  [
    'silero-vad',
    'vad',
    'Silero VAD 6.2.3 (op18, bez If)',
    'MIT',
    11,
    ['silero_vad-6.2.3-py3-none-any.whl'],
    true,
    true,
    [
      'Wykrywanie mowy. Z paczki PyPI, hash przypięty.',
      'Voice activity detection. From the PyPI package, pinned hash.',
    ],
  ],
  [
    'openwakeword-features',
    'wake',
    'openWakeWord 0.5.1: melspektrogram + embedding',
    'Apache-2.0',
    16,
    ['openwakeword-0.5.1-py3-none-any.whl'],
    true,
    true,
    [
      'Cechy słów wywoławczych; klasyfikator PL — własny trening.',
      'Wake-word features; the PL classifier needs own training.',
    ],
  ],
  [
    'wespeaker-resnet34',
    'speaker',
    'WeSpeaker ResNet34 (VoxCeleb)',
    'Apache-2.0 (dane VoxCeleb — sprawdzić warunki)',
    26,
    ['wespeaker_en_voxceleb_resnet34.onnx'],
    false,
    false,
    ['Weryfikacja właściciela (embedding mówcy).', 'Owner verification (speaker embedding).'],
  ],
  [
    'sidecar-llama-vulkan',
    'sidecar',
    'llama-server (vulkan)',
    'MIT',
    40,
    ['llama-vulkan.zip'],
    false,
    false,
    [
      'Serwer modeli lokalnych (127.0.0.1). Wersja do potwierdzenia.',
      'Local model server (127.0.0.1). Version to be confirmed.',
    ],
  ],
  [
    'sidecar-llama-cuda',
    'sidecar',
    'llama-server (cuda)',
    'MIT; cudart/cuBLAS — NVIDIA CUDA EULA',
    600,
    ['llama-cuda-12.4.zip', 'cudart-cuda-12.4.zip'],
    false,
    false,
    [
      'Serwer modeli lokalnych na kartach NVIDIA (CUDA 12.4).',
      'Local model server for NVIDIA GPUs (CUDA 12.4).',
    ],
  ],
  [
    'sidecar-llama-cpu',
    'sidecar',
    'llama-server (cpu)',
    'MIT',
    40,
    ['llama-cpu.zip'],
    false,
    false,
    ['Serwer modeli lokalnych na procesorze (zapas).', 'Local model server on the CPU (fallback).'],
  ],
  [
    'sidecar-whisper-cpu',
    'sidecar',
    'whisper-server (CPU)',
    'MIT',
    8,
    ['whisper-bin-x64.zip'],
    false,
    false,
    ['Serwer rozpoznawania mowy na procesorze.', 'Speech recognition server on the CPU.'],
  ],
  [
    'sidecar-whisper-cuda',
    'sidecar',
    'whisper-server (CUDA 12.4)',
    'MIT; cudart/cuBLAS — NVIDIA CUDA EULA',
    430,
    ['whisper-cublas-12.4.0-bin-x64.zip'],
    false,
    false,
    ['Rozpoznawanie mowy na karcie NVIDIA.', 'Speech recognition on NVIDIA GPUs.'],
  ],
  [
    'sidecar-piper',
    'sidecar',
    'piper (2023.11.14-2)',
    'MIT',
    22,
    ['piper_windows_amd64.zip'],
    false,
    false,
    ['Silnik TTS Piper (z espeak-ng-data).', 'Piper TTS engine (with espeak-ng-data).'],
  ],
  [
    'sidecar-pocket-tts',
    'sidecar',
    'Pocket TTS PL (wrapper JSON-lines)',
    'CC-BY-4.0',
    0,
    [],
    false,
    false,
    [
      'Instalacja ręczna: wrapper budowany osobno.',
      'Manual install: the wrapper is built separately.',
    ],
  ],
];

export function seedItem([
  id,
  kind,
  name,
  license,
  mib,
  files,
  pinned,
  confirmed,
  note,
]: Seed): ModelItem {
  const size = mib * MIB;
  return {
    id,
    kind,
    name,
    license,
    source: 'https://huggingface.co/',
    size_bytes: size,
    target: `%LOCALAPPDATA%\\Alfa\\${kind === 'sidecar' ? 'sidecars' : 'models'}\\${id}`,
    files: files.map((f) => ({
      name: f,
      url: `https://example.invalid/${f}`,
      size_bytes: Math.round(size / files.length),
      pinned_sha256: pinned ? fakeSha(f) : null,
      sha256: null,
    })),
    state: 'missing',
    pinned,
    confirmed,
    downloadable: files.length > 0,
    note: { pl: note[0], en: note[1] },
    progress: null,
    error: null,
    active: false,
  };
}
