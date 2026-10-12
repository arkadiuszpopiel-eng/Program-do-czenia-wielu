// Kula stanu rozmowy: czyste przeliczenia — skala pierścienia z poziomu sygnału i to, czyj głos
// pokazać pod stanem. Widok: `components/voice/VoiceOrb.svelte`.
import type { MicState } from '@alfa/ui-kit';
import type { VoiceSpeaker } from '../api/types-agents';

/** Najmniejsza skala pierścienia (cisza albo mikrofon wyłączony). */
export const ORB_RING_MIN = 0.6;

/**
 * Skala pierścienia 0,6–1. Poziom liczy się tylko wtedy, gdy tor głosu pracuje na dźwięku
 * (słucha, słyszy, agentka mówi); wyłączony, wyciszony i przetwarzający mają pierścień w spoczynku.
 */
export function orbRing(state: MicState, level: number): number {
  const reacts = state === 'listening' || state === 'hearing' || state === 'speaking';
  if (!reacts || !Number.isFinite(level)) return ORB_RING_MIN;
  const clamped = Math.max(0, Math.min(1, level));
  return Math.round((ORB_RING_MIN + (1 - ORB_RING_MIN) * clamped) * 100) / 100;
}

/** Kogo podpisać pod stanem: agentkę, gdy mówi; użytkownika, gdy Alfa go słyszy; inaczej nikogo. */
export function orbSpeaker(state: MicState, speaker: VoiceSpeaker): 'agent' | 'user' | 'nobody' {
  if (state === 'speaking' && speaker === 'agent') return 'agent';
  if (state === 'hearing') return 'user';
  return 'nobody';
}
