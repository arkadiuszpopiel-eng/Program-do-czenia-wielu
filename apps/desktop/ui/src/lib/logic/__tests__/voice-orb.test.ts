import { describe, expect, it } from 'vitest';
import { ORB_RING_MIN, liveStripVisible, orbRing, orbSpeaker } from '../voice-orb';

describe('kula stanu rozmowy', () => {
  it('pierścień rośnie z poziomem tylko wtedy, gdy tor pracuje na dźwięku', () => {
    expect(orbRing('listening', 0)).toBe(ORB_RING_MIN);
    expect(orbRing('hearing', 0.5)).toBe(0.8);
    expect(orbRing('speaking', 1)).toBe(1);
    expect(orbRing('hearing', 7)).toBe(1);
    expect(orbRing('hearing', -1)).toBe(ORB_RING_MIN);
    expect(orbRing('hearing', Number.NaN)).toBe(ORB_RING_MIN);
    for (const state of ['off', 'muted', 'dnd', 'processing'] as const) {
      expect(orbRing(state, 0.9)).toBe(ORB_RING_MIN);
    }
  });

  it('pasek na żywo jest tylko wtedy, gdy tor głosu coś robi', () => {
    for (const state of ['hearing', 'processing', 'speaking'] as const) {
      expect(liveStripVisible(state)).toBe(true);
    }
    for (const state of ['off', 'listening', 'muted', 'dnd'] as const) {
      expect(liveStripVisible(state)).toBe(false);
    }
  });

  it('podpisuje agentkę tylko, gdy mówi, a użytkownika, gdy jest słyszany', () => {
    expect(orbSpeaker('speaking', 'agent')).toBe('agent');
    expect(orbSpeaker('speaking', 'nobody')).toBe('nobody');
    expect(orbSpeaker('hearing', 'user')).toBe('user');
    expect(orbSpeaker('hearing', 'nobody')).toBe('user');
    expect(orbSpeaker('listening', 'agent')).toBe('nobody');
    expect(orbSpeaker('off', 'user')).toBe('nobody');
  });
});
