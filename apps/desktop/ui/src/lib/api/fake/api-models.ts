// Atrapa: modele lokalne (`providers-local`) — manifest z jednym modelem domyślnym i pobieranie
// z postępem co 200 ms (zdarzenia `LocalModelProgress`), anulowanie, model raz pobrany zostaje.
import type { AlfaClient } from '../client';
import type { LocalModelInfo } from '../types-hub';
import type { FakeCore } from './core';

const MIB = 1024 * 1024;
const DEFAULT_MODEL = 'bielik-4.5b-v3.0-instruct-q4_k_m';
const SIZE = 2900 * MIB;
const STEP_MS = 200;
const STEPS = 10;

export function modelsApi(core: FakeCore): AlfaClient['models'] {
  let installed = false;
  let timer: number | null = null;
  let step = 0;

  const info = (): LocalModelInfo => ({
    id: DEFAULT_MODEL,
    name: 'Bielik 4.5B v3.0 Instruct (Q4_K_M)',
    size_bytes: SIZE,
    installed,
    default: true,
    downloading: timer !== null,
  });

  const progress = (state: 'downloading' | 'done' | 'cancelled', bytes: number): void =>
    core.emit([
      {
        type: 'LocalModelProgress',
        model_id: DEFAULT_MODEL,
        state,
        bytes,
        total: SIZE,
        error: null,
      },
    ]);

  const tick = (): void => {
    step++;
    if (step >= STEPS) {
      timer = null;
      installed = true;
      progress('done', SIZE);
      return;
    }
    progress('downloading', Math.round((SIZE * step) / STEPS));
    timer = core.scheduler.setTimeout(tick, STEP_MS);
  };

  return {
    localList: () => core.reply([info()]),
    localDownload: (modelId) => {
      if (modelId !== null && modelId !== DEFAULT_MODEL) {
        return Promise.reject(new Error(`Nieznany model lokalny „${modelId}”.`));
      }
      if (installed) progress('done', SIZE);
      else if (timer === null) {
        step = 0;
        progress('downloading', 0);
        timer = core.scheduler.setTimeout(tick, STEP_MS);
      }
      return core.reply(undefined);
    },
    localCancel: () => {
      if (timer !== null) {
        core.scheduler.clearTimeout(timer);
        timer = null;
        progress('cancelled', Math.round((SIZE * step) / STEPS));
      }
      return core.reply(undefined);
    },
  };
}
