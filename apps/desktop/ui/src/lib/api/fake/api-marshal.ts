// Atrapa: Reguły Marszałka (polecenie → propozycja z podglądem zawężenia → zatwierdzenie tylko
// przez użytkownika, cofnięcie, raport dnia) i karty zgodności mostów CLI (wykrycie, wersja
// przypięta, zgoda na harmonogram, „Zaloguj w terminalu" — polecenie do skopiowania).
import type { AlfaClient } from '../client';
import type { BridgeCard, MarshalProposalInfo, MarshalRuleInfo } from '../types-tasks';
import type { FakeCore } from './core';
import type { FakeTasks } from './api-tasks';

const rule = (id: string, description: string, when: string[], effects: string[]) => ({
  id,
  description,
  when,
  effects,
  rule: { id, description, when: {}, then: effects.map((e) => ({ effect: e })) },
});

/** Szkic reguły z polecenia (atrapa tłumacza — prawdziwy działa przez Router). */
function translate(text: string): MarshalRuleInfo {
  const t = text.toLowerCase();
  if (t.includes('most') || t.includes('claude') || t.includes('codex'))
    return rule('bez-mostow', 'Bez mostów CLI', ['zawsze'], ['mosty CLI zabronione']);
  if (t.includes('noc') || t.includes('cisz'))
    return rule('cisza-nocna', 'Cisza nocna', ['zawsze'], ['cisza 22:00–07:00']);
  if (t.includes('naraz') || t.includes('równoleg'))
    return rule('jedno-naraz', 'Jedno zadanie naraz', ['zawsze'], ['najwyżej 1 zadanie naraz']);
  return rule(
    'zatwierdzaj-powloke',
    'Powłoka za zgodą',
    ['zawsze'],
    ['shell.exec wymaga zatwierdzenia'],
  );
}

export function marshalApi(core: FakeCore, tasks: FakeTasks): AlfaClient['marshal'] {
  let rules: MarshalRuleInfo[] = [
    rule('glos-pierwszy', 'Głos ma pierwszeństwo', ['gdy mówisz'], ['pauza agentek GUI/audio']),
  ];
  const proposals: MarshalProposalInfo[] = [];
  let next = 1;
  const effective = (): string[] => [
    'autonomia najwyżej L2',
    `najwyżej ${rules.some((r) => r.id === 'jedno-naraz') ? 1 : 4} zadania naraz`,
    rules.some((r) => r.id === 'bez-mostow') ? 'mosty CLI zabronione' : 'mosty CLI dozwolone',
  ];
  const find = (id: number) => proposals.find((p) => p.id === id);
  const setStatus = (id: number, status: string): void => {
    const i = proposals.findIndex((p) => p.id === id);
    const p = proposals[i];
    if (p) proposals[i] = { ...p, status };
  };
  return {
    state: () =>
      core.reply({
        rules,
        proposals: [...proposals].reverse(),
        effective: effective(),
        translator: true,
      }),
    propose: (text, drafts) => {
      if (!text.trim() && !drafts)
        return Promise.reject(new Error('Napisz polecenie dla Marszałka.'));
      const fromDrafts = (drafts ?? []).map((d) => {
        const o = d as { id?: unknown; then?: unknown };
        return typeof o.id === 'string' && Array.isArray(o.then)
          ? {
              ok: rule(
                o.id,
                o.id,
                ['zawsze'],
                o.then.map((x) => JSON.stringify(x)),
              ),
            }
          : { bad: { draft: d, errors: ['brak `id` albo `then`'] } };
      });
      const proposal: MarshalProposalInfo = {
        id: next++,
        text,
        rules: drafts
          ? fromDrafts.flatMap((r) => ('ok' in r && r.ok ? [r.ok] : []))
          : [translate(text)],
        rejected: fromDrafts.flatMap((r) => ('bad' in r && r.bad ? [r.bad] : [])),
        conflicts: [],
        status: 'pending',
        created_at: core.isoNow(),
      };
      proposals.push(proposal);
      return core.reply(proposal);
    },
    approve: (id) => {
      const p = find(id);
      if (!p || p.status !== 'pending') return Promise.reject(new Error('Propozycja nie czeka.'));
      rules = [...rules.filter((r) => !p.rules.some((n) => n.id === r.id)), ...p.rules];
      setStatus(id, 'approved');
      return core.reply(p.rules);
    },
    reject: (id) => core.reply(setStatus(id, 'rejected')),
    revoke: (id) => {
      rules = rules.filter((r) => r.id !== id);
      return core.reply(undefined);
    },
    report: () => {
      const done = tasks.tasks.filter((t) => t.state === 'done');
      const ok = done.filter((t) => t.result === 'succeeded').length;
      const failed = done.filter((t) => t.result === 'failed').length;
      return core.reply({
        day: core.isoNow().slice(0, 10),
        submitted: tasks.tasks.length,
        succeeded: ok,
        failed,
        escalations: 0,
        text: `Dziś: ${tasks.tasks.length} zadań, ${ok} udanych, ${failed} nieudanych.`,
      });
    },
  };
}

function seedCards(): BridgeCard[] {
  const card = (
    patch: Partial<BridgeCard> & Pick<BridgeCard, 'route_id' | 'name'>,
  ): BridgeCard => ({
    bridge: null,
    provider: 'anthropic',
    mode: 'cli-p',
    program: null,
    detected: false,
    path: null,
    version: null,
    pinned: [],
    registry_pin: null,
    version_ok: false,
    status: 'gray',
    stale: false,
    verified_at: '2026-09-15',
    sources: [],
    allowed: [],
    forbidden: [],
    enabled: false,
    can_enable: false,
    schedule_per_day: 0,
    login_command: null,
    ...patch,
  });
  return [
    card({
      route_id: 'claude-code-cli',
      name: 'Claude Code (CLI)',
      bridge: 'claude_code',
      program: 'claude',
      detected: true,
      path: 'C:\\Users\\Ty\\AppData\\Roaming\\npm\\claude.cmd',
      version: '2.1.3',
      status: 'green',
      sources: [
        {
          url: 'https://docs.anthropic.com/claude-code',
          quote: 'Tryb nieinteraktywny `claude -p` dla skryptów i automatyzacji.',
        },
      ],
      allowed: ['tryb -p na własnym koncie', 'praca na kopii katalogu'],
      forbidden: ['udostępnianie konta', 'odsprzedaż dostępu'],
      enabled: true,
      can_enable: true,
      login_command: 'claude /login',
    }),
    card({
      route_id: 'codex-cli',
      name: 'Codex CLI',
      bridge: 'codex',
      provider: 'openai',
      program: 'codex',
      can_enable: true,
      login_command: 'codex login',
    }),
    card({ route_id: 'kimi-code-cli', name: 'Kimi Code CLI', provider: 'moonshot' }),
    card({
      route_id: 'agy-antigravity',
      name: 'Antigravity CLI (agy)',
      provider: 'google',
      status: 'forbidden',
      forbidden: ['automatyzacja poza IDE'],
    }),
  ];
}

export function bridgesApi(core: FakeCore): AlfaClient['bridges'] {
  let cards = seedCards();
  const update = (
    id: string,
    patch: (c: BridgeCard) => Partial<BridgeCard>,
  ): Promise<BridgeCard> => {
    const found = cards.find((c) => c.route_id === id || c.bridge === id);
    if (!found) return Promise.reject(new Error(`Nieznany most „${id}”.`));
    let next: BridgeCard;
    try {
      next = { ...found, ...patch(found) };
    } catch (e) {
      return Promise.reject(e instanceof Error ? e : new Error(String(e)));
    }
    cards = cards.map((c) => (c.route_id === found.route_id ? next : c));
    return core.reply(next);
  };
  return {
    list: () => core.reply(cards),
    setEnabled: (routeId, enabled) => {
      const c = cards.find((x) => x.route_id === routeId);
      if (c && !c.can_enable && enabled) {
        return Promise.reject(
          new Error('Tej trasy nie da się włączyć (zabroniona albo nieobsługiwana).'),
        );
      }
      return update(routeId, () => ({ enabled }));
    },
    setSchedule: (bridge, perDay) =>
      update(bridge, () => ({ schedule_per_day: Math.max(0, Math.min(24, Math.round(perDay))) })),
    pin: (bridge, version) =>
      update(bridge, (c) => {
        if (version !== null && version !== c.version) {
          throw new Error('Przypiąć można tylko wersję wykrytą na tym komputerze.');
        }
        const pinned = version === null ? [] : [version];
        return { pinned, version_ok: c.version !== null && pinned.includes(c.version) };
      }),
    openLogin: (bridge) => {
      const c = cards.find((x) => x.bridge === bridge);
      if (!c?.login_command) return Promise.reject(new Error(`Nieznany most „${bridge}”.`));
      return core.reply({ command: c.login_command, cwd: 'C:\\Users\\Ty', opened: true });
    },
  };
}
