// Atrapa: dobór modelu według trudności (`routing_get` / `routing_set` / `routing_preview`).
// Modele to kolejki i modele domyślne kont atrapy; szczeble i ocena trudności — uproszczone
// reguły rdzenia (`logic/routing.ts`). Komunikaty odmowy jak w rdzeniu.
import type { AlfaClient } from '../client';
import { CommandError } from '../command-error';
import type {
  Account,
  ModelRouting,
  ModelRoutingSettings,
  RoutingModel,
  RoutingPreview,
  RoutingRung,
} from '../types-hub';
import { TASK_CLASSES } from '../../logic/model-queue';
import {
  DIFFICULTY_CLASSES,
  estimate,
  levelOf,
  orderByRung,
  rungByName,
  rungFor,
} from '../../logic/routing';
import { FAKE_CATALOG } from './catalog';
import type { FakeCore } from './core';

type RoutingApi = Pick<AlfaClient['accounts'], 'routing' | 'setRouting' | 'routingPreview'>;

const DEFAULTS: ModelRoutingSettings = {
  enabled: true,
  strategy: 'balanced',
  escalate: true,
  review: 'easy',
  local_first: true,
  rungs: {},
  reorder_queues: false,
};

const STATE = new WeakMap<FakeCore, ModelRoutingSettings>();
const PREVIEW_MAX = 20_000;

const providerName = (a: Account) =>
  FAKE_CATALOG.find((p) => p.id === a.provider_id)?.display_name ?? a.provider_id;

/** Modele klasy: kolejka konta albo model domyślny, w kolejności kont. */
function classModels(accounts: readonly Account[], taskClass: string): [Account, string][] {
  return accounts
    .filter((a) => a.state !== 'disabled')
    .flatMap((a) => {
      const queue = a.queues?.[taskClass] ?? [];
      const models = queue.length ? queue : a.auto_model ? [a.auto_model] : [];
      return models.map((m): [Account, string] => [a, m]);
    });
}

function model(a: Account, name: string, settings: ModelRoutingSettings): RoutingModel {
  const candidate = `${a.provider_id}:${name}`;
  const automatic = rungByName(name);
  return {
    candidate,
    provider_name: providerName(a),
    model: name,
    local: false,
    rung: settings.rungs[candidate] ?? automatic,
    automatic,
  };
}

export function routingApi(core: FakeCore): RoutingApi {
  const settings = () => STATE.get(core) ?? DEFAULTS;
  const refuse = (message: string) => Promise.reject(new CommandError('invalid_input', message));
  const view = (): ModelRouting => {
    const s = settings();
    const seen = new Set<string>();
    const models: RoutingModel[] = [];
    for (const taskClass of DIFFICULTY_CLASSES)
      for (const [a, m] of classModels(core.accounts, taskClass)) {
        const item = model(a, m, s);
        if (!seen.has(item.candidate)) {
          seen.add(item.candidate);
          models.push(item);
        }
      }
    return { settings: s, models };
  };
  return {
    routing: () => core.reply(view()),
    setRouting: (next) => {
      const bad = Object.keys(next.rungs).find((k) => !/^[a-z0-9_-]+:\S+$/.test(k.trim()));
      if (bad !== undefined) return refuse(`„${bad}” nie jest modelem w zapisie dostawca:model.`);
      STATE.set(core, { ...next, rungs: { ...next.rungs } });
      return core.reply(view());
    },
    routingPreview: (text, taskClass) => {
      if (!TASK_CLASSES.some((c) => c === taskClass))
        return refuse(`Nieznana klasa zadań „${taskClass}”.`);
      if ([...text].length > PREVIEW_MAX)
        return refuse(`Przykład jest za długi (najwyżej ${PREVIEW_MAX} znaków).`);
      const s = settings();
      const list = classModels(core.accounts, taskClass).map(([a, m]) => model(a, m, s));
      const active = s.enabled && (DIFFICULTY_CLASSES as readonly string[]).includes(taskClass);
      if (!active) {
        const none: RoutingPreview = {
          difficulty: null,
          score: null,
          reasons: '',
          start: null,
          kept_queue: false,
          order: list,
        };
        return core.reply(none);
      }
      const { score, reasons: why } = estimate(text);
      let level = levelOf(score, s.strategy);
      let reasons = why;
      if (taskClass !== 'chat' && level === 'easy') {
        level = 'medium';
        reasons += taskClass === 'code' ? '; zadanie kodu' : '; klasa zadań';
      }
      const kept =
        !s.reorder_queues && core.accounts.some((a) => (a.queues?.[taskClass]?.length ?? 0) > 0);
      const start: RoutingRung = rungFor(level);
      const preview: RoutingPreview = {
        difficulty: level,
        score,
        reasons,
        start,
        kept_queue: kept,
        order: kept ? list : orderByRung(list, start, s.local_first),
      };
      return core.reply(preview);
    },
  };
}
