// Atrapa: wtyczki Wasm (Ustawienia → „Wtyczki”). Deterministyczna: biblioteka startuje z jedną
// zainstalowaną wtyczką („licznik-slow”) i jedną propozycją Ulepszacza („kurs-walut”, sieć do NBP,
// propozycja R2) oraz jednym problemem (przerwanie przez piaskownicę). Cykl życia jak w rdzeniu:
// zatwierdzenie i ponowne włączenie tylko z hashem przejrzanej wersji, wyższa wersja zastępuje
// zainstalowaną, usunięcie kasuje wszystkie wersje. Hashe — pseudo-SHA-256 (FNV), nie kryptografia.
import type { PluginsApi } from '../client-plugins';
import type {
  PluginCapabilityView,
  PluginInfo,
  PluginInspection,
  PluginLimitsView,
  PluginProblem,
  PluginToolView,
  PluginsView,
} from '../types-plugins';
import type { FakeCore } from './core';

const LIMITS: PluginLimitsView = {
  memory_mib: 16,
  fuel_per_call: 50_000_000,
  wall_ms: 2_000,
  max_input_bytes: 65_536,
  max_output_bytes: 65_536,
  max_host_calls: 16,
};

/** Pseudo-SHA-256 (64 znaki hex) — deterministyczny skrót atrapy. */
export function fakeSha(text: string): string {
  let out = '';
  for (let round = 0; out.length < 64; round++) {
    let h = 0x811c9dc5 ^ round;
    for (let i = 0; i < text.length; i++) {
      h ^= text.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    out += h.toString(16).padStart(8, '0');
  }
  return out.slice(0, 64);
}

function base64Bytes(b64: string): Uint8Array {
  const raw = atob(b64.trim());
  return Uint8Array.from(raw, (c) => c.charCodeAt(0));
}

const tool = (name: string, title: string, description: string, mutating = false) =>
  ({ name: `plugin_${name}`, title, description, mutating }) satisfies PluginToolView;

function seed(at: string): PluginInfo[] {
  const counter: PluginInfo = {
    id: 'licznik-slow',
    version: '1.0.0',
    author: 'Właściciel',
    description: 'Liczy słowa w tekście — w piaskownicy Wasm, bez dostępu do plików i sieci.',
    state: 'installed',
    origin: 'user',
    wasm_sha256: fakeSha('licznik-slow@1.0.0:wasm'),
    review_hash: fakeSha('licznik-slow@1.0.0'),
    capabilities: [],
    limits: LIMITS,
    tools: [
      tool('word_count', 'Licznik słów', 'Liczy słowa w podanym tekście (także polskie znaki).'),
    ],
    side_effects: false,
    proposed_at: at,
    decided_at: at,
    r2: null,
  };
  const fx: PluginInfo = {
    id: 'kurs-walut',
    version: '0.2.0',
    author: 'Ulepszacz (propozycja P-12)',
    description: 'Pobiera średni kurs walut z tabeli A NBP i przelicza kwoty.',
    state: 'proposed',
    origin: 'improver',
    wasm_sha256: fakeSha('kurs-walut@0.2.0:wasm'),
    review_hash: fakeSha('kurs-walut@0.2.0'),
    capabilities: [{ family: 'net.egress', scope: 'api.nbp.pl' }],
    limits: { ...LIMITS, max_host_calls: 4 },
    tools: [
      tool(
        'fx_rate',
        'Kurs waluty',
        'Zwraca średni kurs waluty z tabeli A NBP na wskazany dzień.',
        true,
      ),
    ],
    side_effects: true,
    proposed_at: at,
    decided_at: null,
    r2: {
      key: 'plugins.kurs_walut.version',
      value: fakeSha('kurs-walut@0.2.0'),
      from_version: null,
      added_capabilities: ['net.egress'],
    },
  };
  return [fx, counter];
}

interface ManifestShape {
  readonly id?: unknown;
  readonly version?: unknown;
  readonly author?: unknown;
  readonly description?: unknown;
  readonly wasm_sha256?: unknown;
  readonly capabilities?: unknown;
  readonly tools?: unknown;
}

const text = (v: unknown): string => (typeof v === 'string' ? v : '');

function capabilities(raw: unknown): PluginCapabilityView[] {
  if (!Array.isArray(raw)) return [];
  return raw.map((c: { cap?: unknown; scope?: unknown }) => {
    const scope = c.scope as { path?: unknown } | string | undefined;
    return {
      family: text(c.cap),
      scope: typeof scope === 'string' ? scope : text(scope?.path),
    };
  });
}

export class FakePlugins {
  private plugins: PluginInfo[];
  private readonly problems: PluginProblem[];

  constructor(private readonly core: FakeCore) {
    const at = core.isoNow();
    this.plugins = seed(at);
    this.problems = [
      {
        plugin: 'licznik-slow',
        version: '1.0.0',
        kind: 'trapped',
        detail: 'word_count: out_of_fuel',
        at,
      },
    ];
  }

  private view(): PluginsView {
    return {
      available: true,
      unavailable_reason: null,
      plugins: this.plugins,
      problems: this.problems,
    };
  }

  private find(id: string, version?: string): PluginInfo {
    const found = this.plugins.find((p) => p.id === id && (version ? p.version === version : true));
    if (!found) throw new Error(`Wtyczki: nie ma wtyczki \`${id}${version ? `@${version}` : ''}\``);
    return found;
  }

  private update(target: PluginInfo, patch: Partial<PluginInfo>): PluginInfo {
    const next = { ...target, ...patch };
    this.plugins = this.plugins.map((p) => (p === target ? next : p));
    return next;
  }

  private inspectBytes(b64: string): PluginInspection {
    let bytes: Uint8Array;
    try {
      bytes = base64Bytes(b64);
    } catch {
      throw new Error('Wtyczki: moduł nie jest poprawnym base64.');
    }
    const component =
      bytes.length >= 8 &&
      bytes[0] === 0 &&
      bytes[1] === 0x61 &&
      bytes[2] === 0x73 &&
      bytes[3] === 0x6d &&
      bytes[6] === 1;
    return {
      ok: component,
      wasm_sha256: fakeSha(b64),
      bytes: bytes.length,
      error: component
        ? null
        : 'to nie jest komponent Wasm (wymagany komponent świata `alfa:plugin/plugin`)',
    };
  }

  private propose(manifest: unknown, wasmB64: string): PluginInfo {
    const m = (manifest ?? {}) as ManifestShape;
    const id = text(m.id);
    const version = text(m.version);
    if (!/^[a-z][a-z0-9-]{1,47}$/.test(id) || !/^\d+\.\d+\.\d+/.test(version)) {
      throw new Error('Wtyczki: manifest niepoprawny: wymagane `id` i `version` (semver).');
    }
    const caps = capabilities(m.capabilities);
    const forbidden = caps.find((c) => !['fs.read', 'fs.write', 'net.egress'].includes(c.family));
    if (forbidden)
      throw new Error(`Wtyczki: wtyczka nie może deklarować zdolności \`${forbidden.family}\``);
    const inspection = this.inspectBytes(wasmB64);
    if (!inspection.ok) throw new Error(`Wtyczki: ${inspection.error ?? ''}`);
    if (this.plugins.some((p) => p.id === id && p.version === version)) {
      throw new Error(`Wtyczki: wersja ${version} już istnieje z inną treścią — podnieś wersję`);
    }
    const tools = Array.isArray(m.tools)
      ? m.tools.map(
          (t: { name?: unknown; title?: unknown; description?: unknown; mutating?: unknown }) =>
            tool(text(t.name), text(t.title), text(t.description), t.mutating === true),
        )
      : [];
    const side = caps.some((c) => c.family !== 'fs.read');
    const installed = this.plugins.find((p) => p.id === id && p.state === 'installed');
    const hash = fakeSha(`${id}@${version}:${inspection.wasm_sha256}`);
    const info: PluginInfo = {
      id,
      version,
      author: text(m.author) || 'Właściciel',
      description: text(m.description),
      state: 'proposed',
      origin: 'user',
      wasm_sha256: inspection.wasm_sha256,
      review_hash: hash,
      capabilities: caps,
      limits: LIMITS,
      tools: tools.map((t) => ({ ...t, mutating: t.mutating || side })),
      side_effects: side,
      proposed_at: this.core.isoNow(),
      decided_at: null,
      r2: {
        key: `plugins.${id.replaceAll('-', '_')}.version`,
        value: hash,
        from_version: installed?.version ?? null,
        added_capabilities: [...new Set(caps.map((c) => c.family))],
      },
    };
    this.plugins = [info, ...this.plugins];
    return info;
  }

  private approve(id: string, version: string, hash: string): PluginInfo {
    const target = this.find(id, version);
    if (target.state !== 'proposed')
      throw new Error(`Wtyczki: operacja niedozwolona w stanie ${target.state}`);
    if (target.review_hash !== hash) {
      throw new Error(
        'Wtyczki: zatwierdzenie dotyczy innej wersji (hash przejrzanego manifestu niezgodny)',
      );
    }
    for (const old of this.plugins.filter((p) => p.id === id && p.state === 'installed')) {
      this.update(old, { state: 'superseded', decided_at: this.core.isoNow() });
    }
    return this.update(this.find(id, version), {
      state: 'installed',
      decided_at: this.core.isoNow(),
      r2: null,
    });
  }

  private setState(id: string, from: PluginInfo['state'], to: PluginInfo['state']): PluginInfo {
    const target = this.plugins.find((p) => p.id === id && p.state === from);
    if (!target) throw new Error(`Wtyczki: nie ma wtyczki \`${id}\` w stanie ${from}`);
    return this.update(target, { state: to, decided_at: this.core.isoNow() });
  }

  api(): PluginsApi {
    const core = this.core;
    return {
      list: () => core.reply(this.view()),
      inspect: async (wasmB64) => core.reply(this.inspectBytes(wasmB64)),
      propose: async (manifest, wasmB64) => core.reply(this.propose(manifest, wasmB64)),
      approve: async (id, version, hash) => core.reply(this.approve(id, version, hash)),
      reject: async (id, version) => {
        const target = this.find(id, version);
        if (target.state !== 'proposed')
          throw new Error(`Wtyczki: operacja niedozwolona w stanie ${target.state}`);
        return core.reply(
          this.update(target, { state: 'rejected', decided_at: core.isoNow(), r2: null }),
        );
      },
      disable: async (id) => core.reply(this.setState(id, 'installed', 'disabled')),
      enable: async (id, hash) => {
        const target = this.plugins.find((p) => p.id === id && p.state === 'disabled');
        if (!target) throw new Error(`Wtyczki: nie ma wyłączonej wtyczki \`${id}\``);
        if (target.review_hash !== hash) {
          throw new Error(
            'Wtyczki: zatwierdzenie dotyczy innej wersji (hash przejrzanego manifestu niezgodny)',
          );
        }
        return core.reply(this.update(target, { state: 'installed', decided_at: core.isoNow() }));
      },
      remove: async (id) => {
        this.find(id);
        this.plugins = this.plugins.filter((p) => p.id !== id);
        return core.reply(this.view());
      },
    };
  }
}
