// Atrapa areny (`turns_arena_answer`, `turns_select_branch`): odpowiedź przypiętego modelu jako
// nowa gałąź pod wiadomością użytkownika (append-only), z modelem i dostawcą w `Usage`. Treść i
// długość zależą od modelu — porównanie czasu, tokenów i kosztu ma co pokazać. Scenariusz
// `arena-budget`: odpowiedź modeli OpenAI kończy się limitem kosztów (jak w rdzeniu — błąd tury).
import type { AlfaClient } from '../client';
import { CommandError } from '../command-error';
import type { TurnError } from '../types';
import type { FakeChat } from './api-chat';
import { FAKE_CATALOG } from './catalog';
import type { FakeCore } from './core';
import type { ResponseScript } from './responses';

type ArenaApi = Pick<AlfaClient['turns'], 'arenaAnswer' | 'selectBranch'>;

const MODEL_RE = /^([a-z0-9_-]+):(\S+)$/;

const SHORT =
  'W skrócie: **tak**, warto. Najpierw zrób listę zadań na tydzień, potem przypisz im godziny.';
const LONG =
  'Rozpisałam to krok po kroku.\n\n1. Zbierz wszystkie zadania w jednym miejscu.\n2. Oceń, ' +
  'które są pilne, a które ważne.\n3. Zaplanuj bloki skupienia rano i krótkie sprawy po południu.\n\n' +
  'Na koniec tygodnia przejrzyj plan i przenieś to, czego nie udało się skończyć.';

const providerName = (provider: string): string =>
  provider === 'local'
    ? 'Model lokalny'
    : (FAKE_CATALOG.find((p) => p.id === provider)?.display_name ?? provider);

function budgetError(provider: string): TurnError {
  return {
    code: 'budget_blocked',
    message: 'Limit kosztów osiągnięty (100 %) — zmień limit w Ustawieniach → Koszty.',
    retry_at: null,
    provider: providerName(provider),
  };
}

export function arenaTurnsApi(core: FakeCore, chat: FakeChat): ArenaApi {
  return {
    arenaAnswer: (sessionId, turnId, model) => {
      const match = MODEL_RE.exec(model.trim());
      const provider = match?.[1];
      const name = match?.[2];
      const target = core.findTurn(turnId);
      const user =
        target?.author === 'user'
          ? target
          : target?.parent_id
            ? core.findTurn(target.parent_id)
            : undefined;
      if (!provider || !name) {
        return Promise.reject(
          new CommandError(
            'invalid_input',
            `„${model}” nie jest modelem w zapisie dostawca:model (Arena porównuje konkretne modele).`,
          ),
        );
      }
      if (!user || user.author !== 'user') {
        return Promise.reject(
          new CommandError('invalid_input', 'Tura bez wiadomości użytkownika.'),
        );
      }
      // Rdzeń kończy (anuluje) poprzednią generację sesji przed nową.
      chat.streamer.cancel(sessionId);
      const short = (provider.length + name.length) % 2 === 1;
      const script: ResponseScript = {
        agent: 'alfa',
        role_id: 'conductor',
        text: short ? SHORT : LONG,
        thinkingMs: 0,
        tools: [],
        approval: null,
        truncated: false,
        model: { provider: providerName(provider), model: name },
      };
      const failure =
        core.scenario === 'arena-budget' && provider === 'openai' ? budgetError(provider) : null;
      return core.reply(chat.respond(sessionId, user.id, script, null, failure));
    },
    selectBranch: (_sessionId, turnId) =>
      core.findTurn(turnId)
        ? core.reply(undefined)
        : Promise.reject(new CommandError('not_found', 'Nie znaleziono tury.')),
  };
}
