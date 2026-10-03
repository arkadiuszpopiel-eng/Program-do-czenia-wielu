import { describe, expect, it } from 'vitest';
import { FAKE_NEXT, FAKE_STEP_MS, FAKE_TOTAL } from '../fake/api-updates';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import type { AlfaEvent } from '../types-system';
import type { UpdatePhase } from '../types-updates';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario?: 'offline') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  const phases = () =>
    events.flatMap((e) => (e.type === 'UpdateStatus' ? [e.status.phase] : [])) as UpdatePhase[];
  return { client, scheduler, events, phases };
}

describe('atrapa: aktualizacje', () => {
  it('sprawdź → pobierz (postęp) → zweryfikuj → gotowa → restart → „Co nowego" raz', async () => {
    const { client, scheduler, phases, events } = setup();
    expect((await client.updates.status()).phase).toBe('idle');
    await expect(client.updates.download()).rejects.toThrow(/najpierw sprawdź/);
    const found = await client.updates.check();
    expect(found.phase).toBe('available');
    expect(found.available?.version).toBe(FAKE_NEXT);
    expect(found.last_check).not.toBeNull();
    const started = await client.updates.download();
    expect(started.phase).toBe('downloading');
    scheduler.advance(FAKE_STEP_MS * 2);
    await flush();
    const progress = events.flatMap((e) =>
      e.type === 'UpdateStatus' && e.status.progress ? [e.status.progress.downloaded] : [],
    );
    expect(progress.at(-1)).toBe(FAKE_TOTAL / 2);
    scheduler.advance(FAKE_STEP_MS * 6);
    await flush();
    const ready = await client.updates.status();
    expect([ready.phase, ready.ready, ready.previous]).toEqual(['ready', FAKE_NEXT, '0.1.0-f1']);
    expect(phases()).toEqual(
      expect.arrayContaining(['available', 'downloading', 'verifying', 'installing', 'ready']),
    );
    expect(await client.updates.whatsNew()).toBeNull();
    await client.updates.restart();
    const after = await client.updates.status();
    expect([after.current, after.ready, after.previous]).toEqual([FAKE_NEXT, null, '0.1.0-f1']);
    const news = await client.updates.whatsNew();
    expect(news?.version).toBe(FAKE_NEXT);
    await client.updates.dismissWhatsNew();
    expect(await client.updates.whatsNew()).toBeNull();
  });

  it('przerwanie i wznowienie pobierania; restart czeka na koniec rozmowy głosowej', async () => {
    const { client, scheduler } = setup();
    await client.updates.check();
    await client.updates.download();
    scheduler.advance(FAKE_STEP_MS);
    const cancelled = await client.updates.cancel();
    expect(cancelled.phase).toBe('available');
    expect(cancelled.progress?.downloaded).toBe(FAKE_TOTAL / 4);
    await client.updates.download();
    scheduler.advance(FAKE_STEP_MS * 10);
    await client.voice.setMicEnabled(true);
    const blocked = await client.updates.status();
    expect(blocked.restart_blocked?.pl).toMatch(/rozmowa głosowa/);
    await expect(client.updates.restart()).rejects.toThrow(/rozmowa głosowa/);
    await client.voice.setMicEnabled(false);
    await client.updates.restart();
  });

  it('przywrócenie poprzedniej wersji i anulowanie przygotowanej aktualizacji', async () => {
    const { client, scheduler } = setup();
    await expect(client.updates.rollback()).rejects.toThrow(/Brak poprzedniej/);
    await client.updates.check();
    await client.updates.download();
    scheduler.advance(FAKE_STEP_MS * 10);
    const cancelled = await client.updates.rollback();
    expect([cancelled.phase, cancelled.ready]).toEqual(['idle', null]);
    await client.updates.check();
    await client.updates.download();
    scheduler.advance(FAKE_STEP_MS * 10);
    await client.updates.restart();
    const back = await client.updates.rollback();
    expect([back.phase, back.ready, back.previous]).toEqual(['ready', '0.1.0-f1', FAKE_NEXT]);
  });

  it('„O programie": wersja, kanał z ustawień, licencje; offline = błąd sprawdzania', async () => {
    const { client } = setup();
    await client.settings.set('updates.channel', 'preview');
    const about = await client.updates.about();
    expect(about.channel).toBe('beta');
    expect(about.licenses.some((l) => l.source === 'cargo')).toBe(true);
    expect(about.licenses.some((l) => l.source === 'npm')).toBe(true);
    const offline = setup('offline');
    await expect(offline.client.updates.check()).rejects.toThrow(/serwerem wydań/);
    expect((await offline.client.updates.status()).phase).toBe('failed');
  });
});
