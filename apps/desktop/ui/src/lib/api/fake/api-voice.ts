// Atrapa głosu: urządzenia, test mikrofonu (poziom ≤ 30/s), tryb rozmowy (pigułka: słucha →
// słyszy z transkryptem częściowym → przetwarza → agentka mówi), PTT, wyciszenie, stan trybu.
import type { AlfaClient } from '../client';
import type { VoiceStatus } from '../types-agents';
import type { VoicePillState } from '../types-system';
import type { FakeCore } from './core';

const PARTIAL = ['Co', 'Co mam', 'Co mam jutro', 'Co mam jutro w planie?'];

function status(core: FakeCore): VoiceStatus {
  if (core.status.mic !== 'ok') {
    return {
      state: 'unavailable',
      reason: {
        pl: 'Głos niedostępny: brak dostępu do mikrofonu.',
        en: 'Voice unavailable: no microphone access.',
      },
      missing: ['mikrofon'],
      mode: core.voice.mode,
      muted: core.voice.muted,
      agent: 'alfa',
    };
  }
  return {
    state: core.voice.active ? 'active' : 'off',
    reason: null,
    missing: [],
    mode: core.voice.mode,
    muted: core.voice.muted,
    agent: 'alfa',
  };
}

export function voiceApi(core: FakeCore): AlfaClient['voice'] {
  let timer: number | null = null;
  let phase = 0;
  let talk: number | null = null;
  const tick = (): void => {
    phase++;
    // Deterministyczna „mowa": obwiednia sinusoidalna, 30 kl./s.
    const level = Math.max(0, Math.sin(phase / 4) * 0.6 + Math.sin(phase / 1.7) * 0.25);
    core.emit([{ type: 'MicLevel', level: Math.min(1, level) }]);
    timer = core.scheduler.setTimeout(tick, 33);
  };
  const pill = (patch: Partial<VoicePillState>): void => {
    const state: VoicePillState = {
      agent: 'alfa',
      mic: core.voice.muted ? 'muted' : 'listening',
      level: 0,
      speaker: 'nobody',
      partial: null,
      ...patch,
    };
    core.emit([{ type: 'VoicePill', state }]);
  };
  const announce = (): void => core.emit([{ type: 'VoiceStatusChanged', status: status(core) }]);
  /** Scenariusz rozmowy: użytkownik mówi (transkrypt częściowy) → przetwarzanie → Alfa mówi. */
  const conversation = (): void => {
    let step = 0;
    const next = (): void => {
      if (!core.voice.active) return;
      if (step < PARTIAL.length) {
        pill({ mic: 'hearing', speaker: 'user', level: 0.6, partial: PARTIAL[step] ?? null });
      } else if (step === PARTIAL.length) {
        pill({ mic: 'processing', level: 0.1 });
      } else if (step < PARTIAL.length + 4) {
        pill({ mic: 'speaking', speaker: 'agent', level: 0.5 });
      } else {
        pill({});
        return;
      }
      step++;
      talk = core.scheduler.setTimeout(next, 400);
    };
    next();
  };
  const stopTalk = (): void => {
    if (talk !== null) core.scheduler.clearTimeout(talk);
    talk = null;
  };
  return {
    devices: () =>
      core.reply(
        core.status.mic === 'missing'
          ? []
          : [
              { id: 'mic-1', name: 'Mikrofon (Realtek Audio)', default: true },
              { id: 'mic-2', name: 'Zestaw słuchawkowy USB', default: false },
            ],
      ),
    startMicTest: () => {
      if (timer === null && core.status.mic === 'ok') tick();
      return core.reply(undefined);
    },
    stopMicTest: () => {
      if (timer !== null) core.scheduler.clearTimeout(timer);
      timer = null;
      return core.reply(undefined);
    },
    setMicEnabled: (enabled) => {
      if (enabled && core.status.mic !== 'ok') {
        announce();
        return Promise.reject(new Error('Głos niedostępny: brak dostępu do mikrofonu.'));
      }
      core.voice.active = enabled;
      core.voice.mode = 'toggle';
      stopTalk();
      if (enabled) {
        pill({});
        conversation();
      } else pill({ mic: 'off' });
      announce();
      return core.reply(undefined);
    },
    setMuted: (muted) => {
      core.voice.muted = muted;
      if (core.voice.active) pill({});
      announce();
      return core.reply(undefined);
    },
    stopSpeech: () => {
      stopTalk();
      if (core.voice.active) pill({});
      return core.reply(undefined);
    },
    status: () => core.reply(status(core)),
    ptt: (pressed) => {
      if (core.status.mic !== 'ok') return core.reply(undefined);
      core.voice.mode = 'ptt';
      core.voice.active = pressed;
      stopTalk();
      if (pressed) conversation();
      else pill({ mic: 'off' });
      return core.reply(undefined);
    },
    preview: () => core.reply(undefined),
  };
}
