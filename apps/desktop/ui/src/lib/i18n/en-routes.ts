// “Where your data goes” (Settings → Model selection) — keys must match pl-routes.ts.
import type { Message } from './core';
import type { plRoutes } from './pl-routes';

export const enRoutes: Record<keyof typeof plRoutes, Message> = {
  'routes.title': 'Where your data goes',
  'routes.intro':
    'Every route to a cloud model: which country the provider operates in, whether it may train models on your messages and whether it can serve a private session. The data comes from Alfa’s compliance registry; these rules are not changed here.',
  'routes.onlyEnabled': 'Only enabled routes',
  'routes.list': 'Routes to providers',
  'routes.mode.api': 'API key',
  'routes.mode.cli-p': 'CLI bridge',
  'routes.mode.sdk': 'Agent SDK',
  'routes.where': 'data: {place}',
  'routes.place.unknown': 'country unknown',
  'routes.place.US': 'USA',
  'routes.place.EU': 'European Union',
  'routes.place.CN': 'China',
  'routes.place.SG': 'Singapore',
  'routes.place.UK': 'United Kingdom',
  'routes.disabled': 'disabled',
  'routes.training.no_train': 'The provider states it does not train models on your data.',
  'routes.training.may_train': 'The provider may train models on your data.',
  'routes.training.unknown':
    'Unknown whether the provider trains on your data — Alfa assumes it may.',
  'routes.status.green': 'route verified',
  'routes.status.grey': 'route to be verified',
  'routes.status.forbidden': 'route forbidden',
  'routes.stale': 'entry needs re-checking',
  'routes.unverified': 'catalog entry unverified',
  'routes.tags': 'tags',
  'routes.private.yes': 'Private session: allowed.',
  'routes.private.no': 'Private session: blocked — {why}.',
};
