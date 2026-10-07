import { describe, expect, it } from 'vitest';
import type { DeviceProfile } from '../../api/types-hub';
import type { BundleItemView, ModelBundle } from '../../api/types-models';
import {
  bundleAction,
  bundleItemActions,
  bundlePercent,
  canVerify,
  primaryGpu,
  recommendedBundle,
} from '../bundles';

const text = { pl: 'x', en: 'x' };
const bundle = (patch: Partial<ModelBundle>): ModelBundle => ({
  id: 'b',
  rating: 3,
  name: text,
  summary: text,
  requirements: {
    min_ram_mb: 1,
    min_vram_mb: null,
    min_cpu_cores: null,
    gpu_required: false,
    text,
  },
  items: [],
  size_bytes: 100,
  missing_bytes: 100,
  installed: 0,
  total: 3,
  state: 'not_installed',
  fit: { kind: 'fits', reason: null },
  recommended: false,
  quality: [],
  ...patch,
});
const item = (patch: Partial<BundleItemView>): BundleItemView => ({
  id: 'i',
  name: 'I',
  kind: 'stt',
  state: 'missing',
  size_bytes: 1,
  downloadable: true,
  fallback: false,
  ...patch,
});

describe('pakiety 1–6: logika strony', () => {
  it('główne działanie, weryfikacja i postęp w bajtach', () => {
    expect(bundleAction(bundle({}))).toBe('download');
    expect(bundleAction(bundle({ state: 'partial' }))).toBe('resume');
    expect(bundleAction(bundle({ state: 'corrupt' }))).toBe('repair');
    expect(bundleAction(bundle({ state: 'downloading' }))).toBeNull();
    expect(bundleAction(bundle({ state: 'installed' }))).toBeNull();
    expect(bundleAction(bundle({ state: 'needs_trust' }))).toBeNull();
    expect(canVerify(bundle({ installed: 1, state: 'partial' }))).toBe(true);
    expect(canVerify(bundle({ installed: 1, state: 'downloading' }))).toBe(false);
    expect(canVerify(bundle({}))).toBe(false);
    expect(bundlePercent(bundle({ missing_bytes: 25 }))).toBe(75);
    expect(bundlePercent(bundle({ missing_bytes: 500 }))).toBe(0);
    expect(bundlePercent(bundle({ size_bytes: 0 }))).toBeNull();
    const list = [bundle({ id: 'a' }), bundle({ id: 'r', recommended: true })];
    expect(recommendedBundle(list)?.id).toBe('r');
    expect(recommendedBundle([bundle({})])).toBeNull();
  });

  it('działania na elemencie pakietu', () => {
    expect(bundleItemActions(item({}))).toMatchObject({ download: true, repair: false });
    expect(bundleItemActions(item({ state: 'paused' }))).toMatchObject({
      resume: true,
      repair: true,
    });
    expect(bundleItemActions(item({ state: 'installed' }))).toMatchObject({
      verify: true,
      repair: true,
      download: false,
    });
    expect(bundleItemActions(item({ state: 'corrupt' }))).toMatchObject({ repair: true });
    expect(bundleItemActions(item({ state: 'needs_trust' }))).toMatchObject({
      trust: true,
      repair: false,
    });
    expect(bundleItemActions(item({ downloadable: false }))).toMatchObject({
      manual: true,
      download: false,
      repair: false,
    });
    expect(bundleItemActions(item({ downloadable: false, state: 'external' }))).toMatchObject({
      manual: false,
      verify: true,
    });
  });

  it('główna karta: największa pamięć', () => {
    const gpu = (model: string, vram_mb: number) => ({ vendor: 'X', model, vram_mb, backends: [] });
    const profile = (gpus: ReturnType<typeof gpu>[]) =>
      ({ machine: { gpus } }) as unknown as DeviceProfile;
    expect(primaryGpu(profile([gpu('iGPU', 128), gpu('RTX', 5921)]))?.model).toBe('RTX');
    expect(primaryGpu(profile([]))).toBeNull();
  });
});
