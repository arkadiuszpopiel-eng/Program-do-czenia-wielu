import { describe, expect, it } from 'vitest';
import { ENGINE_STEP_MS } from '../fake/api-engines';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import type { ModelItemState } from '../types-models';
import type { AlfaEvent } from '../types-system';
import { trustHashes } from '../../logic/models';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario?: 'offline') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  const states = (id: string) =>
    events.flatMap((e) =>
      e.type === 'ModelChanged' && e.item.id === id ? [e.item.state] : [],
    ) as ModelItemState[];
  const run = async (ms: number) => {
    scheduler.advance(ms);
    await flush();
  };
  return { client, events, states, run };
}

const E5 = 'multilingual-e5-small';

describe('atrapa: modele i silniki', () => {
  it('pobranie bez przypiętego hasha → zgoda TOFU → instalacja → embedder i przebudowa', async () => {
    const { client, events, states, run } = setup();
    const view = await client.engines.list();
    expect(view.embedder.active).toBe('lexical');
    expect(view.items.find((i) => i.id === 'sidecar-pocket-tts')?.downloadable).toBe(false);
    await expect(client.engines.download('sidecar-pocket-tts')).rejects.toThrow(/ręcznie/);
    expect((await client.engines.download(E5)).state).toBe('downloading');
    await run(ENGINE_STEP_MS * 2);
    const progress = events.flatMap((e) => (e.type === 'ModelProgress' ? [e.done] : []));
    expect(progress.length).toBeGreaterThanOrEqual(2);
    await run(ENGINE_STEP_MS * 4);
    const pending = (await client.engines.list()).items.find((i) => i.id === E5);
    expect(pending?.state).toBe('needs_trust');
    expect(pending?.files.every((f) => f.sha256?.length === 64)).toBe(true);
    await expect(client.engines.activateEmbedder(E5)).rejects.toThrow(/nie jest zainstalowany/);
    await expect(
      client.engines.trustHash(E5, { 'onnx/model.onnx': '0'.repeat(64) }),
    ).rejects.toThrow(/zgoda odrzucona/);
    await client.engines.trustHash(E5, trustHashes(pending!));
    await run(ENGINE_STEP_MS);
    expect(states(E5)).toEqual(
      expect.arrayContaining(['queued', 'downloading', 'needs_trust', 'installing', 'installed']),
    );
    const embedder = await client.engines.activateEmbedder(E5);
    expect([embedder.active, embedder.dims]).toEqual([E5, 384]);
    await expect(client.engines.remove(E5)).rejects.toThrow(/aktywny model/);
    await run(ENGINE_STEP_MS * 5);
    const reindex = await client.engines.reindexStatus();
    expect(reindex.finished && !reindex.running).toBe(true);
    expect(events.some((e) => e.type === 'ReindexStatus' && e.status.running)).toBe(true);
  });

  it('limit 2 równoległych, przerwanie → wznowienie, przypięty hash bez zgody', async () => {
    const { client, run } = setup();
    await client.engines.download('silero-vad');
    await client.engines.download('openwakeword-features');
    expect((await client.engines.download('sidecar-llama-vulkan')).state).toBe('queued');
    await run(ENGINE_STEP_MS);
    const paused = await client.engines.cancel('openwakeword-features');
    expect(paused.state).toBe('paused');
    expect(paused.progress?.done).toBeGreaterThan(0);
    await run(ENGINE_STEP_MS * 6);
    const items = (await client.engines.list()).items;
    expect(items.find((i) => i.id === 'silero-vad')?.state).toBe('installed');
    expect(items.find((i) => i.id === 'sidecar-llama-vulkan')?.state).toBe('needs_trust');
    expect((await client.engines.download('openwakeword-features')).state).toBe('downloading');
    await run(ENGINE_STEP_MS * 6);
    const wake = (await client.engines.list()).items.find((i) => i.id === 'openwakeword-features');
    expect(wake?.state).toBe('installed');
    expect((await client.engines.remove('silero-vad')).state).toBe('missing');
  });

  it('offline: pobieranie kończy się czytelnym błędem', async () => {
    const { client, run } = setup('offline');
    await client.engines.download('silero-vad');
    await run(ENGINE_STEP_MS);
    const item = (await client.engines.list()).items.find((i) => i.id === 'silero-vad');
    expect([item?.state, item?.error]).toEqual(['failed', 'sieć: brak połączenia (atrapa)']);
  });
});
