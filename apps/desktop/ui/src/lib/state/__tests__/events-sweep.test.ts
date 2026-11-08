// Przegląd ogólny zdarzeń rdzenia (docs/plan/45 §6.4): każda próbka zdarzenia z fixture'ów rdzenia
// (`crates/app-core/tests/fixtures/events.json` i `extra.json` — test `dto_roundtrip` wymaga próbki
// dla każdego typu, także dodanego później) trafia do okna głównego bez wyjątku, a każdy typ ma
// obsługę: gałąź w `applyEvent` albo strumień rozmowy (`isChatEvent`). Nowe zdarzenie bez obsługi
// w oknie = czerwony test, zamiast cichego pominięcia.
// katalog-przeglad: zdarzenie
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import events from '../../../../../../../crates/app-core/tests/fixtures/events.json';
import extra from '../../../../../../../crates/app-core/tests/fixtures/extra.json';
import type { AlfaEvent } from '../../api/types-system';
import { isChatEvent } from '../../logic/apply-event';
import { setupApp } from './helpers';

const samples = [...events, ...extra.events] as unknown as AlfaEvent[];
const types = [...new Set(samples.map((e) => e.type))].sort();
const dispatcher = readFileSync(
  fileURLToPath(new URL('../apply-batch.ts', import.meta.url)),
  'utf8',
);

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

describe('Przegląd zdarzeń rdzenia w oknie głównym', () => {
  it('próbki obejmują wszystkie typy zdarzeń (40 jak w COMMANDS.md)', () => {
    expect(types.length).toBeGreaterThanOrEqual(40);
  });

  it('każdy typ ma obsługę w oknie (gałąź applyEvent albo strumień rozmowy)', () => {
    const unhandled = types.filter(
      (type) => !dispatcher.includes(`case '${type}':`) && !isChatEvent({ type }),
    );
    expect(unhandled).toEqual([]);
  });

  it('każda próbka trafia do okna bez wyjątku, a okno dalej działa', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    await advance(0);
    const failures: string[] = [];
    const onRejection = (reason: unknown) =>
      failures.push(`odrzucona obietnica: ${String(reason)}`);
    process.on('unhandledRejection', onRejection);
    try {
      for (const event of samples) {
        try {
          client.core.emit([event]);
          await advance(0);
          await advance(0);
        } catch (error) {
          failures.push(`${event.type}: ${String(error)}`);
        }
      }
    } finally {
      process.off('unhandledRejection', onRejection);
    }
    expect(failures).toEqual([]);
    expect(app.view).not.toBe('error');
  });
});
