// Atrapa: „Gdzie trafiają dane” — kilka tras jak w rejestrze zgodności (USA, UE, Chiny, most
// CLI, trasa wyłączona). Reguły sesji prywatnej jak w `compliance`: CN i „może trenować” blokują,
// tag nieznany traktowany ostrożnie.
import type { DataRoutesApi } from '../client-routes';
import type { DataRoute } from '../types-routes';
import type { FakeCore } from './core';

const route = (
  id: string,
  provider: string,
  mode: string,
  jurisdiction: string[],
  privacy: string[],
  training: string,
  extra: Partial<DataRoute> = {},
): DataRoute => {
  const reason = jurisdiction.includes('CN')
    ? 'sesja prywatna: jurysdykcja CN niedozwolona'
    : training === 'may_train'
      ? 'sesja prywatna: dostawca może trenować na danych'
      : training === 'unknown'
        ? 'sesja prywatna: tag prywatności nieznany (traktowany jak „może trenować”)'
        : 'trasa dozwolona';
  return {
    id,
    provider,
    mode,
    jurisdiction,
    privacy,
    training,
    status: 'green',
    stale: false,
    unverified: false,
    enabled: true,
    standard_allowed: true,
    private_allowed: reason === 'trasa dozwolona',
    private_reason: reason,
    ...extra,
  };
};

const ROUTES: readonly DataRoute[] = [
  route('anthropic.api', 'anthropic', 'api', ['US'], ['unknown'], 'unknown'),
  route('claude-code.cli-p', 'anthropic', 'cli-p', ['US'], ['unknown'], 'unknown'),
  route('deepseek.api', 'deepseek', 'api', ['CN'], ['cn-may-train'], 'may_train'),
  route('mistral.api', 'mistral', 'api', ['EU'], ['eu'], 'no_train'),
  route('openai.api', 'openai', 'api', ['US'], ['unknown'], 'unknown'),
  route('gemini-cli.cli-p', 'google', 'cli-p', ['US'], ['google-personal-may-train'], 'may_train', {
    status: 'grey',
    enabled: false,
    standard_allowed: false,
    private_allowed: false,
    private_reason: 'trasa wyłączona',
  }),
];

export class FakeDataRoutes {
  constructor(private readonly core: FakeCore) {}

  api(): DataRoutesApi {
    return { list: () => this.core.reply({ routes: ROUTES }) };
  }
}
