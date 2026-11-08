import { describe, expect, it } from 'vitest';
import { bundleState } from '../fake/api-bundles';
import { ENGINE_STEP_MS } from '../fake/api-engines';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import type { BundleItemView, ModelItemState } from '../types-models';
import { trustHashes } from '../../logic/models';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario?: 'offline') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const run = async (steps: number) => {
    for (let i = 0; i < steps; i++) {
      scheduler.advance(ENGINE_STEP_MS);
      await flush();
    }
  };
  return { client, run };
}

const row = (state: ModelItemState): BundleItemView => ({
  id: state,
  name: state,
  kind: 'llm',
  state,
  size_bytes: 1,
  downloadable: true,
  fallback: false,
});

describe('atrapa: pakiety 1–6', () => {
  it('maszyna atrapy (Radeon 780M 4 GB): 6 za słaby, 5 na styk, 4 zalecany, silniki Vulkan + CPU', async () => {
    const { client } = setup();
    const bundles = await client.engines.bundles();
    expect(bundles.map((b) => [b.rating, b.fit.kind, b.recommended])).toEqual([
      [6, 'too_weak', false],
      [5, 'tight', false],
      [4, 'fits', true],
      [3, 'fits', false],
      [2, 'fits', false],
      [1, 'fits', false],
    ]);
    const minimal = bundles[5];
    expect(minimal?.items.map((i) => [i.id, i.fallback])).toEqual([
      ['bielik-1.5b-v3.0-instruct-q8_0', false],
      ['sidecar-llama-vulkan', false],
      ['sidecar-llama-cpu', true],
    ]);
    expect(minimal?.state).toBe('not_installed');
    expect(minimal?.missing_bytes).toBe(minimal?.size_bytes);
    for (const b of bundles) expect(b.quality.length).toBeGreaterThan(0);
  });

  it('pobranie pakietu → zgoda TOFU → zainstalowany; weryfikacja; nieznany pakiet → błąd', async () => {
    const { client, run } = setup();
    const started = await client.engines.bundleDownload('bundle-minimal');
    expect(started.state).toBe('downloading');
    await run(12);
    const waiting = (await client.engines.bundles()).find((b) => b.id === 'bundle-minimal');
    expect(waiting?.state).toBe('needs_trust');
    const view = await client.engines.list();
    for (const item of view.items.filter((i) => i.state === 'needs_trust')) {
      await client.engines.trustHash(item.id, trustHashes(item));
    }
    await run(2);
    const done = await client.engines.bundleVerify('bundle-minimal');
    expect([done.state, done.installed, done.total, done.missing_bytes]).toEqual([
      'installed',
      3,
      3,
      0,
    ]);
    await expect(client.engines.bundleDownload('bundle-x')).rejects.toThrow(/Nie ma pakietu/);
  });

  it('„Napraw” pobiera element od nowa; ręcznej pozycji nie naprawia; offline → błąd elementu', async () => {
    const { client, run } = setup();
    await client.engines.download('silero-vad');
    await run(6);
    expect((await client.engines.repair('silero-vad')).state).toBe('downloading');
    await run(6);
    const vad = (await client.engines.list()).items.find((i) => i.id === 'silero-vad');
    expect(vad?.state).toBe('installed');
    await expect(client.engines.repair('sidecar-pocket-tts')).rejects.toThrow(/ręcznie/);

    const offline = setup('offline');
    await offline.client.engines.bundleDownload('bundle-minimal');
    await offline.run(2);
    const failed = (await offline.client.engines.bundles()).find((b) => b.id === 'bundle-minimal');
    expect(failed?.state).toBe('corrupt');
  });

  it('stan pakietu: pierwszeństwo jak w rdzeniu', () => {
    expect(bundleState([row('installed'), row('downloading')])).toBe('downloading');
    expect(bundleState([row('installed'), row('failed')])).toBe('corrupt');
    expect(bundleState([row('needs_trust'), row('missing')])).toBe('needs_trust');
    expect(bundleState([row('installed'), row('external')])).toBe('installed');
    expect(bundleState([row('installed'), row('missing')])).toBe('partial');
    expect(bundleState([row('paused')])).toBe('partial');
    expect(bundleState([row('missing')])).toBe('not_installed');
    expect(bundleState([])).toBe('not_installed');
  });
});
