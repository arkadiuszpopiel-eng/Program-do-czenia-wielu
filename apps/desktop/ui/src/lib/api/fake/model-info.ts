// Atrapa: opis modelu po nazwie i propozycja kolejki. W programie robi to rdzeń
// (`accounts-hub-contract`: `model_use`, `model_tier`, `suggest_queue`) — tu jest uproszczona kopia
// tylko po to, żeby atrapa oddawała dane w tym samym kształcie; interfejs z niej nie korzysta.
import {
  MAX_QUEUE,
  fitsClass,
  type QueueStrategy,
  type TaskClassId,
} from '../../logic/model-queue';
import type { ModelInfo, ModelPurpose, ModelTier } from '../types-hub';

const has = (id: string, word: string): boolean => id.split(/[-_./:]/).includes(word);

export function purposeOf(raw: string): ModelPurpose {
  const id = raw.toLowerCase();
  if (id.includes('embed')) return 'embedding';
  if (id.includes('moderation')) return 'moderation';
  if (id.includes('whisper') || id.includes('transcribe')) return 'transcription';
  if (has(id, 'tts')) return 'speech';
  if (id.includes('realtime') || has(id, 'audio')) return 'realtime';
  if (id.includes('dall-e') || has(id, 'image')) return 'image';
  if (id.includes('sora')) return 'video';
  if (
    ['babbage', 'davinci'].some((w) => id.includes(w)) ||
    has(id, 'instruct') ||
    has(id, 'search')
  )
    return 'other';
  return /^o\d/.test(id) ? 'reasoning' : 'chat';
}

export function tierOf(raw: string): ModelTier {
  const id = raw.toLowerCase();
  if (['nano', 'lite'].some((w) => has(id, w))) return 'tiny';
  if (['mini', 'flash', 'haiku', 'small', 'luna'].some((w) => has(id, w))) return 'small';
  if (['pro', 'opus', 'ultra'].some((w) => has(id, w))) return 'premium';
  return 'standard';
}

const isSnapshot = (id: string): boolean => /-\d{4}-\d{2}-\d{2}$|-\d{8}$|-\d{4}$/.test(id);

/** Model w kształcie DTO rdzenia. */
export function fakeModel(id: string, name: string = id, context: number | null = null): ModelInfo {
  const purpose = purposeOf(id);
  const kinds: ModelInfo['kinds'] =
    purpose === 'chat' || purpose === 'reasoning'
      ? ['chat']
      : purpose === 'embedding'
        ? ['embeddings']
        : purpose === 'speech'
          ? ['tts']
          : purpose === 'transcription'
            ? ['stt']
            : [];
  return {
    id,
    name,
    kinds,
    context_tokens: context,
    purpose,
    tier: tierOf(id),
    snapshot: isSnapshot(id),
  };
}

function rank(model: ModelInfo, strategy: QueueStrategy): number {
  const version = Number(/\d+(\.\d)?/.exec(model.id)?.[0] ?? 0) * 100;
  const size = { premium: 30, standard: 20, small: -40, tiny: -60 }[model.tier];
  const shaped =
    strategy === 'economy'
      ? -size * 6
      : strategy === 'balanced' && model.tier === 'premium'
        ? -40
        : size;
  return version + shaped - (model.purpose === 'reasoning' ? 300 : 0);
}

/** Propozycja kolejki (uproszczona): pasujące modele bez migawek, najlepsze najpierw. */
export function suggestFake(
  models: readonly ModelInfo[],
  taskClass: TaskClassId,
  strategy: QueueStrategy,
): string[] {
  const fitting = models.filter((m) => fitsClass(taskClass, m.purpose));
  const undated = fitting.filter((m) => !m.snapshot);
  return (undated.length >= 3 ? undated : fitting)
    .map((m, at) => ({ m, at, score: rank(m, strategy) }))
    .sort((a, b) => b.score - a.score || a.at - b.at)
    .slice(0, MAX_QUEUE)
    .map(({ m }) => m.id);
}

/** Lista OpenAI zmieszana jak u dostawcy: rozmowa, mowa, transkrypcja, obrazy, embeddingi. */
export const OPENAI_MODELS: readonly ModelInfo[] = [
  fakeModel('tts-1', 'TTS 1'),
  fakeModel('tts-1-hd', 'TTS 1 HD'),
  fakeModel('whisper-1', 'Whisper 1'),
  fakeModel('text-embedding-3-small', 'Embedding 3 Small'),
  fakeModel('text-embedding-3-large', 'Embedding 3 Large'),
  fakeModel('dall-e-3', 'DALL·E 3'),
  fakeModel('gpt-image-1', 'GPT Image 1'),
  fakeModel('omni-moderation-latest', 'Moderation'),
  fakeModel('gpt-6-realtime', 'GPT-6 Realtime'),
  fakeModel('gpt-6-sol', 'GPT-6 Sol', 400_000),
  fakeModel('gpt-6-sol-2026-08-06', 'GPT-6 Sol (2026-08-06)', 400_000),
  fakeModel('gpt-6-luna', 'GPT-6 Luna', 200_000),
  fakeModel('gpt-6-nano', 'GPT-6 Nano', 200_000),
  fakeModel('gpt-6-pro', 'GPT-6 Pro', 400_000),
  fakeModel('gpt-5.5', 'GPT-5.5', 400_000),
  fakeModel('gpt-5.5-mini', 'GPT-5.5 mini', 200_000),
  fakeModel('gpt-5', 'GPT-5', 400_000),
  fakeModel('gpt-5-mini', 'GPT-5 mini', 200_000),
  fakeModel('gpt-5-nano', 'GPT-5 nano', 200_000),
  fakeModel('gpt-4.1', 'GPT-4.1', 1_000_000),
  fakeModel('gpt-4.1-mini', 'GPT-4.1 mini', 1_000_000),
  fakeModel('gpt-4o', 'GPT-4o', 128_000),
  fakeModel('gpt-4o-2024-08-06', 'GPT-4o (2024-08-06)', 128_000),
  fakeModel('gpt-4o-mini', 'GPT-4o mini', 128_000),
  fakeModel('gpt-4o-mini-tts', 'GPT-4o mini TTS'),
  fakeModel('gpt-4o-transcribe', 'GPT-4o Transcribe'),
  fakeModel('gpt-4o-search-preview', 'GPT-4o Search (preview)'),
  fakeModel('o3', 'o3', 200_000),
  fakeModel('o4-mini', 'o4-mini', 200_000),
  fakeModel('gpt-3.5-turbo-instruct', 'GPT-3.5 Turbo Instruct'),
  fakeModel('babbage-002', 'Babbage 002'),
];
