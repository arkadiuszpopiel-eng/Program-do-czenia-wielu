// Dane startowe atrapy (deterministyczne względem `now`): sesje, drzewo tur z wariantami i kartą
// zatwierdzenia, oś czasu, artefakty, profil sprzętu, konta.
import type { AgentId } from '@alfa/ui-kit';
import type { ArtifactInfo, Author, SessionSummary, TimelineEvent, ToolStep, Turn } from '../types';
import type { Account, DeviceProfile } from '../types-hub';
import { renderBlocks } from './render';

const MIN = 60_000;
const iso = (ms: number): string => new Date(ms).toISOString();
const pln = (minor: number) => ({ minor, currency: 'PLN' as const });

export function session(
  id: string,
  title: string,
  updatedAt: number,
  extra: Partial<SessionSummary> = {},
): SessionSummary {
  return {
    id,
    title,
    project: null,
    pinned: false,
    archived: false,
    working: false,
    unread: false,
    updated_at: iso(updatedAt),
    tags: [],
    autonomy: 'L3',
    profile: 'hybrid',
    ...extra,
  };
}

export function turn(
  id: string,
  sessionId: string,
  parent: string | null,
  author: Author,
  text: string,
  at: number,
  extra: Partial<Turn> = {},
): Turn {
  return {
    id,
    session_id: sessionId,
    parent_id: parent,
    author,
    role_id: null,
    created_at: iso(at),
    status: 'complete',
    text,
    blocks: renderBlocks(text, false),
    thinking: null,
    tools: [],
    approval: null,
    usage: null,
    error: null,
    continues: null,
    addressed_to: null,
    truncated: false,
    heard_prefix: null,
    ...extra,
  };
}

const step = (id: string, icon: ToolStep['icon'], label: string, ms: number, undo = false) =>
  ({
    id,
    icon,
    label,
    status: 'done',
    duration_ms: ms,
    // Token dziennika cofania jak w rdzeniu: `<sesja>:u<krok>` (fixture'y to sesja `s-q3`).
    undo_token: undo ? `s-q3:u${id.replace(/\D/g, '')}` : null,
    undone: false,
    intent: null,
  }) as ToolStep;

const usage = (inTok: number, outTok: number, minor: number, ms: number) => ({
  input_tokens: inTok,
  output_tokens: outTok,
  cost: pln(minor),
  latency_ms: ms,
  provider: 'Anthropic',
  model: 'claude-opus-5-5',
});

export function seedSessions(now: number): SessionSummary[] {
  const projectX = { id: 'p-x', name: 'Projekt X' };
  const shop = { id: 'p-shop', name: 'Sklep' };
  return [
    session('s-q3', 'Raport Q3', now - 2 * MIN, { project: projectX, pinned: true, working: true }),
    session('s-api', 'Kod: API płatności', now - 45 * MIN, { project: shop }),
    session('s-shop', 'Zakupy na weekend', now - 3 * 60 * MIN, { unread: true }),
    session('s-trip', 'Plan wyjazdu do Gdańska', now - 26 * 60 * MIN),
    session('s-db', 'Migracja bazy', now - 50 * 60 * MIN, { project: shop }),
    session('s-long', 'Długa rozmowa (1000 wiadomości)', now - 72 * 60 * MIN),
    session('s-old', 'Stare notatki', now - 30 * 24 * 60 * MIN, { archived: true }),
  ];
}

/** Rozmowa „Raport Q3": wczoraj + dziś, warianty 1/2, gałąź, karta zatwierdzenia. */
export function seedQ3(now: number): Turn[] {
  const s = 's-q3';
  const y = now - 24 * 60 * MIN;
  const t = now - 6 * MIN;
  return [
    turn('q1', s, null, 'user', 'Zbierz dane do raportu Q3 z folderu Finanse/2026-Q3.', y),
    turn(
      'q2',
      s,
      'q1',
      'gama',
      'Zebrałam 14 plików. Przychody są w trzech arkuszach, koszty w jednym. Mam gotowe sumy cząstkowe.',
      y + MIN,
      {
        role_id: 'researcher',
        tools: [step('st1', 'search', 'Przeszukano Finanse/2026-Q3 (14 plików)', 820)],
        usage: usage(3200, 410, 21, 1900),
      },
    ),
    turn(
      'q3',
      s,
      'q2',
      'user',
      'Przygotujcie raport Q3 dla zarządu: przychody, koszty i trzy rekomendacje.',
      t,
    ),
    turn(
      'q4a',
      s,
      'q3',
      'gama',
      'Przychody: 4 812 300 zł (+11% r/r). Koszty operacyjne: 3 106 900 zł. Marża wzrosła o 2,4 p.p.',
      t + 1 * MIN,
      {
        role_id: 'researcher',
        thinking: { duration_ms: 4200, active: false },
        tools: [
          step('st2', 'file', 'Odczytano przychody-Q3.xlsx, koszty-Q3.xlsx', 1400),
          step('st3', 'terminal', 'Policzono sumy kontrolne', 610),
        ],
        usage: usage(5400, 380, 34, 2300),
      },
    ),
    turn(
      'q4b',
      s,
      'q3',
      'gama',
      'Zsumowałam przychody z trzech arkuszy: 4 812 300 zł (+11% r/r). Koszty operacyjne wyniosły 3 106 900 zł, a marża wzrosła o 2,4 p.p.\n\nRekomendacje:\n- utrzymać tempo sprzedaży B2B,\n- renegocjować dwie największe umowy dostawców,\n- przesunąć 5% budżetu marketingu do kanałów cyfrowych.\n\n```sql\nSELECT kwartal, SUM(przychod) AS przychod\nFROM finanse\nWHERE rok = 2026\nGROUP BY kwartal;\n```',
      t + 2 * MIN,
      {
        role_id: 'researcher',
        thinking: { duration_ms: 6100, active: false },
        tools: [step('st4', 'file', 'Odczytano 3 arkusze przychodów', 1200)],
        usage: usage(5600, 620, 41, 2800),
      },
    ),
    turn('q5', s, 'q4b', 'user', 'Delta, wstaw to do dokumentu dla zarządu.', t + 3 * MIN, {
      addressed_to: 'delta',
    }),
    turn(
      'q6',
      s,
      'q5',
      'delta',
      'Utworzyłam szkic raport-Q3.docx. Chcę jeszcze nadpisać szablon zarząd.dotx, żeby stopka miała właściwy kwartał — to wymaga Twojej zgody.',
      t + 4 * MIN,
      {
        role_id: 'operator',
        tools: [step('st5', 'edit', 'Utworzono szkic raport-Q3.docx', 2100, true)],
        approval: {
          id: 'ap-1',
          what: 'Nadpisanie szablonu Raporty/zarząd.dotx',
          why: 'Zarząd używa tego szablonu; stopka musi mieć bieżący kwartał.',
          reversible: true,
          risk: 'medium',
          status: 'pending',
          broker_window: true,
          expires_at: null,
        },
        usage: usage(2100, 190, 12, 1500),
      },
    ),
  ];
}

const LONG_TEXTS = [
  'Sprawdziłam wszystkie pozycje i wszystko się zgadza.',
  'Mogę przygotować krótkie podsumowanie albo pełną tabelę — co wolisz?',
  'Zapisałam notatkę w pamięci sesji.',
  'Ta zmiana jest cofalna jednym kliknięciem w dzienniku cofania.',
];

/** 1000 wiadomości — do pomiaru przełączania sesji i wirtualizacji (PLAN §14.7). */
export function seedLong(now: number): Turn[] {
  const out: Turn[] = [];
  const agents: AgentId[] = ['alfa', 'beta', 'gama', 'delta'];
  let parent: string | null = null;
  for (let i = 0; i < 1000; i++) {
    const id = `l${i}`;
    const at = now - (1000 - i) * 7 * MIN;
    const author: Author = i % 2 === 0 ? 'user' : (agents[(i >> 1) % 4] ?? 'alfa');
    const text = author === 'user' ? `Pytanie numer ${i / 2 + 1}.` : (LONG_TEXTS[i % 4] ?? '');
    out.push(
      turn(id, 's-long', parent, author, text, at, {
        role_id: author === 'user' ? null : 'conductor',
      }),
    );
    parent = id;
  }
  return out;
}

export function seedSmall(sessionId: string, now: number, topic: string): Turn[] {
  return [
    turn(`${sessionId}-1`, sessionId, null, 'user', topic, now - 50 * MIN),
    turn(
      `${sessionId}-2`,
      sessionId,
      `${sessionId}-1`,
      'alfa',
      'Jasne — zaczęłam od listy kroków. Daj znać, czy mam od razu działać.',
      now - 49 * MIN,
      {
        role_id: 'conductor',
      },
    ),
  ];
}

export function seedTimeline(now: number): TimelineEvent[] {
  const e = (
    id: string,
    minutesAgo: number,
    kind: TimelineEvent['kind'],
    level: TimelineEvent['level'],
    agent: AgentId | null,
    title: string,
    extra: Partial<TimelineEvent> = {},
  ): TimelineEvent => ({
    id,
    ts: iso(now - minutesAgo * MIN),
    session_id: 's-q3',
    kind,
    level,
    agent,
    title,
    detail: null,
    cost: null,
    latency_ms: null,
    turn_id: null,
    ...extra,
  });
  return [
    e('e1', 5, 'model_call', 'info', 'gama', 'claude-opus-5-5 · 5 600 → 620 tokenów', {
      cost: pln(41),
      latency_ms: 2800,
      turn_id: 'q4b',
    }),
    e('e2', 5, 'tool', 'info', 'gama', 'fs.read · 3 arkusze przychodów', {
      latency_ms: 1200,
      turn_id: 'q4b',
    }),
    e('e3', 3, 'tool', 'info', 'delta', 'fs.write · raport-Q3.docx (cofalne)', {
      latency_ms: 2100,
      turn_id: 'q6',
    }),
    e('e4', 2, 'audit', 'audit', 'delta', 'Prośba o zatwierdzenie: nadpisanie zarząd.dotx', {
      turn_id: 'q6',
      detail: 'Ryzyko średnie · cofalne',
    }),
    e('e5', 2, 'model_call', 'info', 'delta', 'claude-opus-5-5 · 2 100 → 190 tokenów', {
      cost: pln(12),
      latency_ms: 1500,
      turn_id: 'q6',
    }),
    e('e6', 1, 'diagnostics', 'warn', null, 'Kurs NBP z wczoraj — użyto kursu zapasowego'),
  ];
}

export function seedArtifacts(now: number): ArtifactInfo[] {
  return [
    {
      id: 'a1',
      session_id: 's-q3',
      name: 'raport-Q3.docx',
      path: 'C:\\Users\\Ty\\Alfa\\Sesje\\Raport Q3\\out\\raport-Q3.docx',
      size_bytes: 48_213,
      mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      created_at: iso(now - 3 * MIN),
      agent: 'delta',
      versions: 2,
    },
    {
      id: 'a2',
      session_id: 's-q3',
      name: 'sumy-Q3.csv',
      path: 'C:\\Users\\Ty\\Alfa\\Sesje\\Raport Q3\\out\\sumy-Q3.csv',
      size_bytes: 1_904,
      mime: 'text/csv',
      created_at: iso(now - 5 * MIN),
      agent: 'gama',
      versions: 1,
    },
    {
      id: 'a3',
      session_id: 's-q3',
      name: 'notatki.md',
      path: 'C:\\Users\\Ty\\Alfa\\Sesje\\Raport Q3\\out\\notatki.md',
      size_bytes: 612,
      mime: 'text/markdown',
      created_at: iso(now - 20 * MIN),
      agent: 'beta',
      versions: 3,
    },
  ];
}

export function seedDevice(now: number): DeviceProfile {
  return {
    machine: {
      id: 'm-7f3a',
      name: 'DESKTOP-ALFA',
      os: 'Windows 11 Pro 24H2',
      cpu: { model: 'AMD Ryzen 7 7840HS', cores: 8, threads: 16 },
      ram_mb: 32_768,
      gpus: [{ vendor: 'AMD', model: 'Radeon 780M', vram_mb: 4096, backends: ['Vulkan', 'CPU'] }],
      npu: 'AMD XDNA (10 TOPS)',
      battery: { percent: 84, on_ac: true },
    },
    recommendation: {
      hw_class: 'standard_amd',
      voice_profile: 'B',
      llm_backend: 'llama.cpp · Vulkan',
      tradeoffs: [
        {
          pl: 'Lokalny model 3–4,5B na GPU; większe modele przez chmurę.',
          en: 'Local 3–4.5B model on GPU; larger models via cloud.',
        },
        {
          pl: 'Na baterii rozpoznawanie mowy przełącza się na lżejszy model.',
          en: 'On battery, speech recognition switches to a lighter model.',
        },
      ],
    },
    measured_at: iso(now - 2 * 24 * 60 * MIN),
  };
}

export function seedAccounts(now: number): Account[] {
  return [
    {
      id: 'acc-anthropic',
      provider_id: 'anthropic',
      label: 'Anthropic — prywatne',
      state: 'ok',
      key_stored: true,
      models: [
        {
          id: 'claude-opus-5-5',
          name: 'Claude Opus 5.5',
          kinds: ['chat', 'vision'],
          context_tokens: 1_000_000,
        },
        { id: 'claude-haiku-5', name: 'Claude Haiku 5', kinds: ['chat'], context_tokens: 200_000 },
      ],
      assignments: {
        task_classes: ['chat', 'code'],
        agents: ['alfa', 'gama', 'delta'],
        voice_stt: false,
        voice_tts: false,
      },
      cost_limit: { enabled: true, monthly: pln(20_000) },
      last_tested_at: iso(now - 3 * 60 * MIN),
    },
  ];
}
