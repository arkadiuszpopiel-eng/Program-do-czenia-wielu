// Atrapa: biblioteka umiejętności (zainstalowana, propozycja nowej wersji z diffem, kwarantanna
// importu z zewnątrz, propozycja z pamięci; instalacja tylko z hashem przejrzanej wersji,
// uruchomienie = zadanie agentki) i Kreator agentek (rozmowa → szkic, podgląd z odmianą imienia,
// test na sucho, zapis tylko po zaliczonym teście tego samego hasha).
import type { AlfaClient } from '../client';
import type {
  AgentDraft,
  BuilderAgentInfo,
  BuilderDryRun,
  BuilderPreview,
  DiffLine,
  SkillInfo,
  SkillReview,
} from '../types-work';
import type { FakeCore } from './core';

/** Deterministyczny „hash" (FNV-1a, 64 znaki hex) — atrapa SHA-256. */
export function fakeHash(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0').repeat(8);
}

const SCHEMA = {
  type: 'object',
  properties: { folder: { type: 'string' } },
  required: ['folder'],
  additionalProperties: false,
};

export class FakeSkills {
  skills: SkillInfo[] = [];
  private readonly prompts = new Map<string, string>();

  constructor(
    private readonly core: FakeCore,
    private readonly tasks: AlfaClient['tasks'],
  ) {
    if (core.scenario === 'empty' || core.scenario === 'first-run') return;
    const at = (h: number) => new Date(core.scheduler.now() - h * 3_600_000).toISOString();
    const skill = (patch: Partial<SkillInfo> & Pick<SkillInfo, 'id' | 'version'>): SkillInfo => {
      const s: SkillInfo = {
        name: 'Porządki w Pobranych',
        description: 'Sortuje pliki w folderze według typu (PDF, obrazy, archiwa).',
        state: 'installed',
        origin: 'user',
        trusted: true,
        hash: '',
        findings: [],
        keywords: ['porządki', 'pobrane', 'pliki'],
        required_tools: ['fs_list', 'fs_move', 'fs_mkdir'],
        required_capabilities: ['fs.read', 'fs.write'],
        parameters: SCHEMA,
        proposed_at: at(48),
        decided_at: at(47),
        ...patch,
      };
      return { ...s, hash: fakeHash(`${s.id}@${s.version}:${s.description}`) };
    };
    this.skills = [
      skill({
        id: 'porzadki-pobrane',
        version: '1.1.0',
        state: 'proposed',
        description: 'Sortuje pliki w folderze według typu i usuwa puste podfoldery.',
        required_tools: ['fs_list', 'fs_move', 'fs_mkdir', 'fs_delete'],
        proposed_at: at(1),
        decided_at: null,
      }),
      skill({ id: 'porzadki-pobrane', version: '1.0.0' }),
      skill({
        id: 'import-z-sieci',
        version: '0.1.0',
        name: 'Import z sieci',
        description: 'Pobiera plik i rozpakowuje go w katalogu sesji.',
        state: 'quarantined',
        origin: 'external',
        trusted: false,
        findings: ['polecenie sieciowe w kroku 2 (Invoke-WebRequest)'],
        required_tools: ['shell_run'],
        required_capabilities: ['shell.exec'],
        proposed_at: at(3),
        decided_at: null,
      }),
      skill({
        id: 'raport-tygodniowy',
        version: '1.0.0',
        name: 'Raport tygodniowy',
        description: 'Zbiera notatki z tygodnia w jeden dokument.',
        state: 'proposed',
        origin: 'memory',
        keywords: ['raport', 'notatki'],
        required_tools: ['fs_read', 'fs_write'],
        proposed_at: at(5),
        decided_at: null,
      }),
    ];
  }

  private find(id: string, version: string): SkillInfo | undefined {
    return this.skills.find((s) => s.id === id && s.version === version);
  }

  private set(next: SkillInfo): SkillInfo {
    this.skills = this.skills.map((s) =>
      s.id === next.id && s.version === next.version ? next : s,
    );
    this.core.emit([{ type: 'SkillsChanged', skill_id: next.id }]);
    return next;
  }

  private review(s: SkillInfo): SkillReview {
    const prev = this.skills.find((x) => x.id === s.id && x.state === 'installed' && x !== s);
    const lines = (x: SkillInfo | undefined) =>
      x ? JSON.stringify({ description: x.description, tools: x.required_tools }, null, 2) : '';
    const before = lines(prev).split('\n');
    const after = lines(s).split('\n');
    const diff: DiffLine[] = [];
    for (const l of after) diff.push({ kind: before.includes(l) ? 'same' : 'added', text: l });
    for (const l of before) if (!after.includes(l)) diff.push({ kind: 'removed', text: l });
    return { skill: s, previous_version: prev?.version ?? null, diff };
  }

  api(): AlfaClient['skills'] {
    const core = this.core;
    const missing = () => Promise.reject(new Error('Umiejętność nie istnieje.'));
    const decide = (id: string, version: string, hash: string, from: SkillInfo['state']) => {
      const s = this.find(id, version);
      if (!s) return missing();
      if (s.state !== from) return Promise.reject(new Error(`Umiejętność ma stan ${s.state}.`));
      if (s.hash !== hash.trim())
        return Promise.reject(new Error('Hash nie zgadza się z przejrzaną wersją.'));
      for (const old of this.skills.filter((x) => x.id === id && x.state === 'installed'))
        this.set({ ...old, state: 'superseded' });
      return core.reply(this.set({ ...s, state: 'installed', decided_at: core.isoNow() }));
    };
    return {
      list: () => core.reply([...this.skills]),
      review: (id, version) => {
        const s = this.find(id, version);
        return s ? core.reply(this.review(s)) : missing();
      },
      propose: (raw) => {
        const r = raw as Partial<SkillInfo> & { id?: string; version?: string; prompt?: string };
        if (!r.id || !r.version || !r.name)
          return Promise.reject(new Error('Umiejętność: brak id, wersji albo nazwy.'));
        const s: SkillInfo = {
          id: r.id,
          version: r.version,
          name: r.name,
          description: r.description ?? '',
          state: 'proposed',
          origin: 'user',
          trusted: true,
          hash: fakeHash(`${r.id}@${r.version}:${r.description ?? ''}`),
          findings: [],
          keywords: r.keywords ?? [],
          required_tools: r.required_tools ?? [],
          required_capabilities: r.required_capabilities ?? [],
          parameters: r.parameters ?? { type: 'object', properties: {} },
          proposed_at: core.isoNow(),
          decided_at: null,
        };
        this.skills = [s, ...this.skills];
        this.prompts.set(s.id, r.prompt ?? '');
        core.emit([{ type: 'SkillsChanged', skill_id: s.id }]);
        return core.reply(s);
      },
      approve: (id, version, hash) => decide(id, version, hash, 'proposed'),
      release: (id, version, hash) => decide(id, version, hash, 'quarantined'),
      reject: (id, version) => {
        const s = this.find(id, version);
        return s
          ? core.reply(this.set({ ...s, state: 'rejected', decided_at: core.isoNow() }))
          : missing();
      },
      disable: (id) => {
        const s = this.skills.find((x) => x.id === id && x.state === 'installed');
        return s ? core.reply(this.set({ ...s, state: 'disabled' })) : missing();
      },
      run: (id, sessionId, agent, params) => {
        const s = this.skills.find((x) => x.id === id && x.state === 'installed');
        if (!s) return Promise.reject(new Error(`Umiejętność „${id}” nie jest zainstalowana.`));
        const folder = (params as { folder?: unknown } | null)?.folder;
        if (typeof folder !== 'string' || !folder)
          return Promise.reject(new Error('Parametry umiejętności: brak „folder”.'));
        return this.tasks.create({
          session_id: sessionId,
          title: `Umiejętność: ${s.name}`,
          goal: `${s.description} Folder: ${folder}`,
          agent,
          after: [],
          parent_id: null,
        });
      },
      exportBundle: () =>
        core.reply({
          status: 'saved' as const,
          path: 'C:\\Users\\ala\\Documents\\umiejetnosci-alfa.skills.json',
          files: this.skills.filter((s) => s.state === 'installed').length,
          bytes: 2_048,
        }),
      importBundle: () => core.reply({ proposed: [], skipped: [] }),
    };
  }
}

/** Odmiana imienia żeńskiego (przybliżenie atrapy; rdzeń liczy ją w `agent-builder-contract`). */
const FORMS = (name: string): string[] => {
  if (name.endsWith('ia')) {
    const stem = name.slice(0, -1);
    return [name, `${stem}i`, `${stem}i`, `${stem}ę`, `${stem}ą`, `${stem}i`, `${stem}o`];
  }
  const stem = name.endsWith('a') ? name.slice(0, -1) : name;
  const soft = /[kg]$/.test(stem) ? 'i' : 'y';
  return [name, `${stem}${soft}`, `${stem}ie`, `${stem}ę`, `${stem}ą`, `${stem}ie`, `${stem}o`];
};

export class FakeBuilder {
  private passed = new Set<string>();
  private library: BuilderAgentInfo[] = [];

  constructor(private readonly core: FakeCore) {}

  private build(draft: AgentDraft): BuilderPreview {
    const name = (draft.name ?? '').trim();
    if (!/^\p{Lu}\p{Ll}{1,31}$/u.test(name))
      throw new Error('Kreator: imię (litery, wielka pierwsza).');
    if (['Alfa', 'Beta', 'Gama', 'Delta'].includes(name))
      throw new Error(`Kreator: imię „${name}” jest już zajęte.`);
    const role = draft.role;
    if (!role || !role.name.trim()) throw new Error('Kreator: rola bez nazwy.');
    const autonomy = draft.limits.autonomy ?? 'L3';
    if (autonomy === 'L4')
      throw new Error('Kreator: L4 włącza się tylko przełącznikiem w Ustawieniach.');
    const id = name.toLowerCase();
    const groups = [...role.tools];
    const preview: Omit<BuilderPreview, 'hash'> = {
      persona_id: id,
      name,
      forms: FORMS(name),
      glyph: draft.glyph ?? name[0] ?? '?',
      color: draft.color ?? 'color.agent.custom-1',
      character: draft.character ?? '',
      role_id: role.id || `${id}-rola`,
      role_name: role.name,
      groups,
      read_only: role.read_only,
      tools: groups.flatMap((g) => (g === 'fs' ? ['fs_list', 'fs_read', 'fs_move'] : [`${g}_*`])),
      voice: `${draft.voice?.base ?? 'pl-f1'} · wysokość ${(draft.voice?.pitch ?? 1).toFixed(2)} · tempo ${(draft.voice?.rate ?? 1).toFixed(2)}`,
      autonomy,
      system_prompt: `Jesteś ${name}. ${role.prompt}`.trim(),
      fs_write: [...draft.limits.fs_write],
      memory_scope: draft.limits.memory_scope ?? 'agent',
      retain_days: draft.limits.retain_days ?? 30,
      max_steps: draft.limits.budget?.max_steps ?? 40,
      warnings: draft.limits.fs_write.length
        ? []
        : ['Brak zakresu zapisu — agentka tylko czyta pliki.'],
    };
    return { ...preview, hash: fakeHash(JSON.stringify(preview)) };
  }

  api(): AlfaClient['builder'] {
    const core = this.core;
    const guard = <T>(f: () => T): Promise<T> => {
      try {
        return core.reply(f());
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    };
    return {
      policy: () =>
        core.reply({
          groups: ['fs', 'fs.read', 'shell', 'memory', 'gui.control', 'web', 'delegate'],
          ceiling: 'L3' as const,
          palette: Array.from({ length: 8 }, (_, i) => `color.agent.custom-${i + 1}`),
          voices: ['pl-f1', 'pl-f2', 'pl_PL-gosia-medium'],
          model_policies: ['conversation', 'planning', 'code', 'research', 'summarize'],
          max_steps: 200,
        }),
      propose: (description) => {
        const name = /o imieniu (\p{Lu}\p{Ll}+)/u.exec(description)?.[1] ?? null;
        const files = /plik|folder|pobran/i.test(description);
        const draft: AgentDraft = {
          id: null,
          name,
          forms: null,
          glyph: null,
          color: null,
          character: null,
          voice: {
            base: 'pl-f2',
            pitch: 1.05,
            rate: 1,
            perceived_age: 22,
            timbre: 'ciepła',
            design_prompt: '',
          },
          role: {
            id: '',
            name: files ? 'Porządkowa' : 'Pomocniczka',
            description: description.slice(0, 200),
            prompt: 'Pomagam porządkować pliki i zawsze mówię, co zrobiłam.',
            model_policy: 'conversation',
            tools: files ? ['fs'] : ['memory'],
            read_only: false,
            untrusted_isolated: false,
            author: false,
          },
          limits: {
            autonomy: 'L2',
            budget: null,
            fs_write: files ? ['%USERPROFILE%\\Downloads\\**'] : [],
            memory_scope: 'agent',
            retain_days: 30,
            triggers: [],
          },
          skills: [],
        };
        const questions = name ? [] : ['Jak ma mieć na imię?'];
        return core.reply({ draft, questions });
      },
      preview: (draft) => guard(() => this.build(draft)),
      dryRun: (draft) =>
        guard((): BuilderDryRun => {
          const p = this.build(draft);
          const write = p.fs_write.length > 0 && p.groups.includes('fs');
          const steps: BuilderDryRun['steps'] = [
            { tool: 'fs_list', expected: 'allowed', outcome: 'allowed', why: 'odczyt' },
            ...(write
              ? [
                  {
                    tool: 'fs_move',
                    expected: p.autonomy === 'L1' ? ('ask' as const) : ('allowed' as const),
                    outcome: p.autonomy === 'L1' ? ('ask' as const) : ('allowed' as const),
                    why: 'w zakresie i poziomie autonomii',
                  },
                ]
              : []),
            {
              tool: 'fs_move',
              expected: 'denied',
              outcome: 'denied',
              why: 'zapis poza zakresem agentki',
            },
            {
              tool: 'shell_run',
              expected: 'denied',
              outcome: 'denied',
              why: 'rola nie ma tego narzędzia',
            },
          ];
          this.passed.add(p.hash);
          return { hash: p.hash, passed: true, steps };
        }),
      save: (draft, hash) =>
        guard(() => {
          const p = this.build(draft);
          if (p.hash !== hash || !this.passed.has(hash))
            throw new Error('Najpierw wykonaj test na sucho tego szkicu.');
          this.library = [
            ...this.library.filter((a) => a.persona !== p.persona_id),
            {
              persona: p.persona_id,
              name: p.name,
              color: p.color,
              role: p.role_name,
              autonomy: p.autonomy,
              hash,
            },
          ];
          return { persona: p.persona_id, role: p.role_id, hash };
        }),
      voicePreview: () => core.reply(undefined),
      library: () => core.reply([...this.library]),
    };
  }
}
