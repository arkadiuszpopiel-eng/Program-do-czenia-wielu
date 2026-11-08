// Wspólne narzędzia testów stanu: atrapa backendu z wirtualnym zegarem i ręcznymi klatkami.
import { FakeAlfaClient, VirtualScheduler, type FakeOptions } from '../../api/fake/fake-client';
import { ManualFrames } from '../../logic/raf-batcher';
import { AppState } from '../app.svelte';

export const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

export function setupApp(options: Omit<FakeOptions, 'scheduler'> = {}) {
  const scheduler = new VirtualScheduler();
  const frames = new ManualFrames();
  const client = new FakeAlfaClient({ scheduler, ...options });
  const app = new AppState(client, { frames, debounceMs: 0 });
  /** Czas atrapy → mikrozadania (zdarzenia) → klatka (paczka do stanu). */
  const advance = async (ms: number): Promise<void> => {
    scheduler.advance(ms);
    await flush();
    frames.tick();
  };
  return { app, client, scheduler, frames, advance };
}

export function keyEvent(
  key: string,
  code: string,
  mods: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>> = {},
) {
  let prevented = false;
  return {
    key,
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    isComposing: false,
    target: null,
    ...mods,
    get defaultPrevented() {
      return prevented;
    },
    preventDefault() {
      prevented = true;
    },
  } as unknown as KeyboardEvent;
}
