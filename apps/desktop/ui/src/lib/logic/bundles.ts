// Logika pakietów 1–6 na stronie „Modele i silniki” (bez Svelte — testowana w vitest): główne
// działanie pakietu, postęp w bajtach, działania na elemencie, opis sprzętu tej maszyny.
import type { DeviceProfile, GpuInfo } from '../api/types-hub';
import type { BundleItemView, ModelBundle } from '../api/types-models';

/** Główne działanie pakietu (`null` — nic do zrobienia albo trwa pobieranie / czeka na zgodę). */
export type BundleAction = 'download' | 'resume' | 'repair' | null;

export function bundleAction(bundle: ModelBundle): BundleAction {
  switch (bundle.state) {
    case 'not_installed':
      return 'download';
    case 'partial':
      return 'resume';
    case 'corrupt':
      return 'repair';
    default:
      return null;
  }
}

/** „Sprawdź pliki” ma sens, gdy coś jest zainstalowane i nic się nie pobiera. */
export function canVerify(bundle: ModelBundle): boolean {
  return bundle.installed > 0 && bundle.state !== 'downloading';
}

/** Procent pobranych bajtów pakietu (0–100) albo `null` przy pustym pakiecie. */
export function bundlePercent(bundle: ModelBundle): number | null {
  if (bundle.size_bytes <= 0) return null;
  const done = bundle.size_bytes - Math.min(bundle.missing_bytes, bundle.size_bytes);
  return Math.max(0, Math.min(100, Math.floor((done / bundle.size_bytes) * 100)));
}

/** Pakiet zalecany (najwyższy pasujący bez kompromisów) albo `null`. */
export function recommendedBundle(bundles: readonly ModelBundle[]): ModelBundle | null {
  return bundles.find((b) => b.recommended) ?? null;
}

export interface BundleItemActions {
  readonly download: boolean;
  readonly resume: boolean;
  /** Usuń pliki i pobierz od nowa. */
  readonly repair: boolean;
  /** Ponowne SHA-256. */
  readonly verify: boolean;
  /** Pliki bez przypiętego SHA-256 — zgoda na karcie w katalogu. */
  readonly trust: boolean;
  /** Instalacja ręczna według opisu w katalogu. */
  readonly manual: boolean;
}

export function bundleItemActions(item: BundleItemView): BundleItemActions {
  const s = item.state;
  const present = s === 'installed' || s === 'external' || s === 'corrupt';
  const can = item.downloadable;
  return {
    download: can && (s === 'missing' || s === 'failed'),
    resume: can && s === 'paused',
    repair: can && (present || s === 'failed' || s === 'paused'),
    verify: present,
    trust: s === 'needs_trust',
    manual: !can && !present,
  };
}

/** Główna karta: największa pamięć (jak `device-profile::Profile::primary_gpu`). */
export function primaryGpu(profile: DeviceProfile): GpuInfo | null {
  let best: GpuInfo | null = null;
  for (const gpu of profile.machine.gpus) {
    if (!best || gpu.vram_mb > best.vram_mb) best = gpu;
  }
  return best;
}
