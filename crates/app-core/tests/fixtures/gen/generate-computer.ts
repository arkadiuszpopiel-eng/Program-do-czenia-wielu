// Fixture'y komend F8 (panel „Ekran", wbudowany terminal, umiejętności, Kreator agentek,
// „Zdrowie systemu") — część generatora `generate.ts` (atrapa + `TauriAlfaClient` z atrapą `invoke`).
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

const DRAFT = {
  id: null,
  name: 'Zofia',
  forms: null,
  glyph: 'Z',
  color: 'color.agent.custom-2',
  character: 'Spokojna, konkretna.',
  voice: {
    base: 'pl-f2',
    pitch: 1.05,
    rate: 1,
    perceived_age: 22,
    timbre: 'ciepła',
    design_prompt: '',
  },
  role: {
    id: 'porzadkowa',
    name: 'Porządkowa',
    description: 'Porządkuje Pobrane.',
    prompt: 'Porządkuję pliki i mówię, co zrobiłam.',
    model_policy: 'conversation',
    tools: ['fs'],
    read_only: false,
    untrusted_isolated: false,
    author: false,
  },
  limits: {
    autonomy: 'L2',
    budget: {
      max_steps: 40,
      max_tokens: 200_000,
      max_wall_ms: 600_000,
      max_cost_micro_usd: null,
      max_tool_calls_per_turn: 8,
    },
    fs_write: ['%USERPROFILE%\\Downloads\\**'],
    memory_scope: 'agent',
    retain_days: 30,
    triggers: [],
  },
  skills: [],
};

export async function runComputer(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  await both('gui', 'status');
  await both('gui', 'screenshot');
  await both('gui', 'stop');
  await both('gui', 'release');
  await both('gui', 'desktopGrant', 's-q3', 'delta');

  const frames: unknown[] = [];
  const term = (await both('terminal', 'open', 'claude_login', 100, 30, null, (f: unknown) =>
    frames.push(f),
  )) as { id: number };
  await both('terminal', 'input', term.id, btoa('/login\r'));
  await both('terminal', 'resize', term.id, 120, 32);
  await both('terminal', 'list');
  await both('terminal', 'close', term.id);
  await flush();

  const skills = (await both('skills', 'list')) as { id: string; version: string; hash: string }[];
  const next = skills.find((s) => s.id === 'porzadki-pobrane' && s.version === '1.1.0');
  await both('skills', 'review', 'porzadki-pobrane', '1.1.0');
  await both('skills', 'approve', 'porzadki-pobrane', '1.1.0', next?.hash ?? '');
  await both('skills', 'propose', {
    id: 'notatki-dnia',
    version: '1.0.0',
    name: 'Notatki dnia',
    description: 'Zbiera notatki dnia w jeden plik.',
    prompt: 'Zbierz notatki z folderu {{folder}}.',
    parameters: { type: 'object', properties: { folder: { type: 'string' } } },
  });
  await both('skills', 'reject', 'notatki-dnia', '1.0.0');
  const quarantined = skills.find((s) => s.id === 'import-z-sieci');
  await both('skills', 'release', 'import-z-sieci', '0.1.0', quarantined?.hash ?? '');
  await both('skills', 'run', 'porzadki-pobrane', 's-q3', 'beta', {
    folder: 'C:\\Users\\ala\\Downloads',
  });
  await both('skills', 'disable', 'import-z-sieci');
  await both('skills', 'exportBundle');
  await both('skills', 'importBundle');
  scheduler.runAll();
  await flush();

  await both('builder', 'policy');
  await both('builder', 'propose', 'Agentka o imieniu Zofia, która porządkuje pobrane pliki.');
  await both('builder', 'preview', DRAFT);
  const dry = (await both('builder', 'dryRun', DRAFT)) as { hash: string };
  await both('builder', 'voicePreview', DRAFT);
  await both('builder', 'save', DRAFT, dry.hash);
  await both('builder', 'library');

  await both('health', 'report');
  await both('health', 'scan');
  await both('health', 'approve', 1);
  const view = (await both('health', 'report')) as { repaired: { id: number }[] };
  await both('health', 'undo', view.repaired[0]?.id ?? 0);
  await both('health', 'reject', 3);
  const improver = (await both('health', 'improver')) as {
    proposals: { id: number; digest: string }[];
  };
  await both('health', 'improverCycle');
  const first = improver.proposals[0];
  await both('health', 'improverApprove', first?.id ?? 0, first?.digest ?? '');
  await both('health', 'improverRollback', first?.id ?? 0);
  await both('health', 'improverReject', improver.proposals[1]?.id ?? 0);
  await both('health', 'evals');
  await both('health', 'evalsVerify', 'F4-agents');
  await flush();
}
