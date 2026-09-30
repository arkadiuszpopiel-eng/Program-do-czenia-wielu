// Atrapa: aplikacja, agentki, koszty, ustawienia, oś czasu, pliki, uprawnienia, urządzenia, głos, system.
import type { AlfaClient } from '../client';
import type { EventLevel } from '../types';
import type { FakeChat } from './api-chat';
import type { FakeCore } from './core';
import { seedDevice } from './fixtures';
import { SETTINGS_SCHEMA, defaultValues } from './settings-schema';

const LEVELS: readonly EventLevel[] = ['trace', 'debug', 'info', 'warn', 'error', 'audit'];

export function appApi(core: FakeCore): AlfaClient['app'] {
  return {
    bootstrap: () =>
      core.reply({
        app_version: '0.1.0-f1',
        locale: core.settings['ui.locale'] === 'en' ? 'en' : 'pl',
        onboarding_done: core.onboardingDone,
        machine_name: 'DESKTOP-ALFA',
        settings: core.settings,
        layout: core.layout,
        active_session_id: core.activeSession,
        shortcut_overrides: core.shortcutOverrides,
      }),
    completeOnboarding: () => {
      core.onboardingDone = true;
      return core.reply(undefined);
    },
    openSystemSettings: () => core.reply(undefined),
    saveLayout: (layout) => {
      core.layout = layout;
      return core.reply(undefined);
    },
    setActiveSession: (id) => {
      core.activeSession = id;
      return core.reply(undefined);
    },
  };
}

export function agentsApi(core: FakeCore, chat: FakeChat): AlfaClient['agents'] {
  return {
    list: (sid) => core.reply(core.agentsOf(sid)),
    setRoles: (sid, agent, roleIds) => core.reply(core.setAgent(sid, agent, { role_ids: roleIds })),
    applyCast: (sid, template) => {
      const cast = chat.castFor(template);
      core.agents[sid] = core.agentsOf(sid).map((a) => ({ ...a, role_ids: cast[a.id] }));
      core.emit([{ type: 'AgentsChanged', session_id: sid, agents: core.agents[sid] ?? [] }]);
      return core.reply(undefined);
    },
  };
}

export function costsApi(core: FakeCore, chat: FakeChat): AlfaClient['costs'] {
  return {
    summary: (sid) => core.reply(chat.costs(sid)),
    setMonthlyLimit: (enabled, monthly) => {
      core.limit = { enabled, monthly: { minor: monthly.minor, currency: 'PLN' } };
      const sid = core.activeSession;
      core.emit([{ type: 'CostsChanged', session_id: sid ?? '', costs: chat.costs(sid) }]);
      return core.reply(undefined);
    },
  };
}

export function settingsApi(core: FakeCore): AlfaClient['settings'] {
  return {
    schema: () => core.reply(SETTINGS_SCHEMA),
    values: () => core.reply(core.settings),
    set: (key, value) => {
      core.settings[key] = value;
      return core.reply(undefined);
    },
    reset: (key) => {
      const value = defaultValues()[key] ?? '';
      core.settings[key] = value;
      return core.reply(value);
    },
    setShortcut: (actionId, chord) => {
      if (chord === null) delete core.shortcutOverrides[actionId];
      else core.shortcutOverrides[actionId] = chord;
      return core.reply(undefined);
    },
  };
}

export function timelineApi(core: FakeCore): AlfaClient['timeline'] {
  return {
    list: (sid, filter) => {
      const min = LEVELS.indexOf(filter.min_level);
      const events = core.timeline.filter(
        (e) =>
          e.session_id === sid &&
          (filter.kinds.length === 0 || filter.kinds.includes(e.kind)) &&
          LEVELS.indexOf(e.level) >= min,
      );
      return core.reply(events);
    },
  };
}

export function filesApi(core: FakeCore): AlfaClient['files'] {
  return {
    list: (sid) => core.reply(core.artifacts.filter((a) => a.session_id === sid)),
    preview: (id) => {
      const artifact = core.artifacts.find((a) => a.id === id);
      if (!artifact) return core.reply({ kind: 'none' as const });
      if (artifact.mime === 'text/csv') {
        return core.reply({
          kind: 'text' as const,
          text: 'segment;przychod;koszty\nB2B;2 914 000;1 802 300\nB2C;1 898 300;1 304 600',
          truncated: false,
        });
      }
      if (artifact.mime === 'text/markdown') {
        return core.reply({
          kind: 'text' as const,
          text: '# Notatki\n\n- sprawdzić kurs EUR z 30.09\n- dopisać notę o różnicy 0,3%',
          truncated: false,
        });
      }
      return core.reply({ kind: 'none' as const });
    },
    act: () => core.reply(undefined),
  };
}

export function permissionsApi(core: FakeCore): AlfaClient['permissions'] {
  return {
    get: (sid) =>
      core.reply({
        global: 'L3' as const,
        session: sid ? (core.session(sid)?.autonomy ?? null) : null,
        hello_enabled: false,
      }),
    requestLevel: () =>
      core.reply({ status: 'opened_broker' as const, request_id: core.nextId('br') }),
    openApproval: () =>
      core.reply({ status: 'opened_broker' as const, request_id: core.nextId('br') }),
  };
}

export function deviceApi(core: FakeCore): AlfaClient['device'] {
  return {
    profile: () => core.reply(seedDevice(core.scheduler.now())),
    measure: () => core.reply(seedDevice(core.scheduler.now())),
  };
}

export function voiceApi(core: FakeCore): AlfaClient['voice'] {
  let timer: number | null = null;
  let phase = 0;
  const tick = (): void => {
    phase++;
    // Deterministyczna „mowa": obwiednia sinusoidalna, 30 kl./s.
    const level = Math.max(0, Math.sin(phase / 4) * 0.6 + Math.sin(phase / 1.7) * 0.25);
    core.emit([{ type: 'MicLevel', level: Math.min(1, level) }]);
    timer = core.scheduler.setTimeout(tick, 33);
  };
  return {
    devices: () =>
      core.reply(
        core.status.mic === 'missing'
          ? []
          : [
              { id: 'mic-1', name: 'Mikrofon (Realtek Audio)', default: true },
              { id: 'mic-2', name: 'Zestaw słuchawkowy USB', default: false },
            ],
      ),
    startMicTest: () => {
      if (timer === null && core.status.mic === 'ok') tick();
      return core.reply(undefined);
    },
    stopMicTest: () => {
      if (timer !== null) core.scheduler.clearTimeout(timer);
      timer = null;
      return core.reply(undefined);
    },
    setMicEnabled: () => core.reply(undefined),
    setMuted: () => core.reply(undefined),
    stopSpeech: () => core.reply(undefined),
  };
}

export function systemApi(core: FakeCore, chat: FakeChat): AlfaClient['system'] {
  return {
    status: () => core.reply(core.status),
    retryQueue: () => {
      if (core.status.online) chat.flushQueue();
      else
        core.emit([
          {
            type: 'Toast',
            kind: 'warning',
            message: { pl: 'Nadal brak połączenia.', en: 'Still offline.' },
          },
        ]);
      return core.reply(undefined);
    },
  };
}
