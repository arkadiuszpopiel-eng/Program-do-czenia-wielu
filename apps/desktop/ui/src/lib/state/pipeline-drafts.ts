import type { PipelineDef } from '../api/types';

/** Szkice tylko w pamięci procesu UI; przełączenie biblioteki i zamknięcie okna ich nie kasuje. */
export interface PipelineDraft {
  draft: PipelineDef;
  original: string | null;
  snapshot: string;
  input: string;
}
export const pipelineDrafts = new Map<string, PipelineDraft>();
export const pipelineDraftKey = (draft: PipelineDef, original: string | null): string =>
  original ?? (draft.builtin ? draft.name : '__new__');
export const pipelineResume = { key: '' };
