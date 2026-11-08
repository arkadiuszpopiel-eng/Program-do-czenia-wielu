// Kolejki modeli konta API: klasy zadań, reguły dopasowania modelu do klasy i operacje na liście.
// Przeznaczenie i wielkość modelu przychodzą z rdzenia (`ModelInfo.purpose`, `tier`) — tu nie ma
// zgadywania po nazwie, tylko to, co wolno z modelem zrobić w interfejsie.
import type { ModelInfo, ModelPurpose } from '../api/types-hub';

/** Najwięcej modeli w kolejce jednej klasy zadań (jak `MAX_QUEUE` w rdzeniu). */
export const MAX_QUEUE = 10;

/** Klasy zadań Routera w kolejności pokazywanej w interfejsie. */
export const TASK_CLASSES = [
  'chat',
  'code',
  'planning',
  'voice_fast',
  'gui_vision',
  'summarize',
  'embeddings',
] as const;
export type TaskClassId = (typeof TASK_CLASSES)[number];

export const STRATEGIES = ['quality', 'balanced', 'economy'] as const;
export type QueueStrategy = (typeof STRATEGIES)[number];

/** Przeznaczenia w kolejności filtrów przeglądarki modeli. */
export const PURPOSES: readonly ModelPurpose[] = [
  'chat',
  'reasoning',
  'embedding',
  'speech',
  'transcription',
  'realtime',
  'image',
  'video',
  'moderation',
  'other',
];

/** Czy model o tym przeznaczeniu nadaje się do klasy zadań (jak `use_fits` w rdzeniu). */
export function fitsClass(taskClass: TaskClassId, purpose: ModelPurpose): boolean {
  if (taskClass === 'embeddings') return purpose === 'embedding';
  // Szybka odpowiedź głosowa i sterowanie ekranem nie czekają na model rozumujący.
  if (taskClass === 'voice_fast' || taskClass === 'gui_vision') return purpose === 'chat';
  return purpose === 'chat' || purpose === 'reasoning';
}

/** Dlaczego modelu nie da się dodać do kolejki (`null` = można). */
export type AddBlock = 'full' | 'present' | 'unfit' | null;

export function addBlock(
  queue: readonly string[],
  taskClass: TaskClassId,
  model: Pick<ModelInfo, 'id' | 'purpose'>,
): AddBlock {
  if (queue.includes(model.id)) return 'present';
  if (!fitsClass(taskClass, model.purpose)) return 'unfit';
  return queue.length >= MAX_QUEUE ? 'full' : null;
}

/** Przesuwa pozycję `index` o `delta` miejsc (poza zakresem — bez zmian). */
export function moved(queue: readonly string[], index: number, delta: number): string[] {
  const target = index + delta;
  const next = [...queue];
  const item = next[index];
  if (item === undefined || target < 0 || target >= next.length) return next;
  next.splice(index, 1);
  next.splice(target, 0, item);
  return next;
}

export interface ModelFilter {
  readonly query: string;
  /** Pusty zbiór = wszystkie przeznaczenia. */
  readonly purposes: ReadonlySet<ModelPurpose>;
  readonly hideSnapshots: boolean;
  /** Tylko modele pasujące do tej klasy zadań (`null` = bez ograniczenia). */
  readonly fitting: TaskClassId | null;
}

/** Modele przechodzące filtr, w kolejności z listy dostawcy. */
export function filterModels(models: readonly ModelInfo[], filter: ModelFilter): ModelInfo[] {
  const words = filter.query.toLowerCase().split(/\s+/).filter(Boolean);
  return models.filter((m) => {
    if (filter.hideSnapshots && m.snapshot) return false;
    if (filter.purposes.size > 0 && !filter.purposes.has(m.purpose)) return false;
    if (filter.fitting && !fitsClass(filter.fitting, m.purpose)) return false;
    const text = `${m.id} ${m.name}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/** Ile modeli ma każde przeznaczenie (do liczników przy filtrach). */
export function countByPurpose(models: readonly ModelInfo[]): Map<ModelPurpose, number> {
  const counts = new Map<ModelPurpose, number>();
  for (const m of models) counts.set(m.purpose, (counts.get(m.purpose) ?? 0) + 1);
  return counts;
}
