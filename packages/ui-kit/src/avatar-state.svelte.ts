// Wspólny stan awatarów agentek: styl (żywe twarze albo same litery) i poziom mowy z głośnika,
// według którego mówiąca agentka rusza ustami. Ustawia go aplikacja; awatary tylko czytają.

export type AvatarStyle = 'animated' | 'letters';

/** Po tylu ms bez nowego pomiaru poziom uznajemy za nieaktualny i awatar rusza się własnym rytmem. */
export const SPEECH_LEVEL_FRESH_MS = 300;

export const avatarState = $state<{
  style: AvatarStyle;
  /** Poziom mowy 0..1 (obwiednia tego, co właśnie gra głośnik). */
  level: number;
  /** Czas ostatniego pomiaru (`performance.now()`); 0 = brak pomiarów. */
  levelAt: number;
}>({ style: 'animated', level: 0, levelAt: 0 });

export function setAvatarStyle(style: unknown): void {
  avatarState.style = style === 'letters' ? 'letters' : 'animated';
}

export function setSpeechLevel(level: number, now: number = performance.now()): void {
  avatarState.level = Math.max(0, Math.min(1, Number.isFinite(level) ? level : 0));
  avatarState.levelAt = now;
}

/** Otwarcie ust 0..1 dla poziomu mowy: cicha mowa też ma być widoczna, głośna nie przesadzona. */
export function mouthOpen(level: number): number {
  const clamped = Math.max(0, Math.min(1, level));
  return clamped <= 0.02 ? 0 : Math.min(1, 0.18 + Math.sqrt(clamped) * 0.82);
}
