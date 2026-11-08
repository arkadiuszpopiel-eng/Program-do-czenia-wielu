// Atrapa: ustawienia dla firm istniejącego konta (`accounts_set_enterprise`, C2.12, 0.0.65) —
// te same reguły co hub kont (`set_enterprise`): sprawdzenie jak przy dodaniu konta (adres zasobu
// Azure zostaje), odmowa, gdy ten sam klucz w tym samym projekcie OpenAI ma już inne konto,
// a konto Azure dostaje modele z wdrożeń (kolejki tracą modele bez wdrożenia).
import type { AlfaClient } from '../client';
import { CommandError } from '../command-error';
import type { Account } from '../types-hub';
import { AZURE_OPENAI, billingProject, checkEnterprise, coreMessage } from '../../logic/enterprise';
import type { FakeCore } from './core';
import { fakeModel, suggestFake } from './model-info';

/** Stan atrapy kont potrzebny do zmiany (klucze i adresy kont żyją tylko w `api-hub.ts`). */
export interface EnterpriseContext {
  readonly keys: ReadonlyMap<string, string>;
  readonly urls: ReadonlyMap<string, string>;
}

/** Komunikat huba jako zdanie (jak `app_modules::accounts_enterprise`): bez „niepoprawne dane:”. */
export function sentence(message: string): string {
  const text = message.replace(/^niepoprawne dane: /, '').trim();
  const out = text.charAt(0).toUpperCase() + text.slice(1);
  return /[.!?]$/.test(out) ? out : `${out}.`;
}

const sameUrl = (a: string, b: string) =>
  a.replace(/\/+$/, '').toLowerCase() === b.replace(/\/+$/, '').toLowerCase();

export function enterpriseApi(
  core: FakeCore,
  ctx: EnterpriseContext,
): AlfaClient['accounts']['setEnterprise'] {
  const refuse = (message: string) =>
    Promise.reject(new CommandError('invalid_input', sentence(message)));
  return (id, enterprise) => {
    const index = core.accounts.findIndex((a) => a.id === id);
    const account = core.accounts[index];
    if (!account) return Promise.reject(new CommandError('not_found', 'Nie ma takiego konta.'));
    const url = ctx.urls.get(id) || null;
    // Konto z danych przykładowych nie ma zapisanego adresu — rdzeń sprawdza adres konta.
    const firm = checkEnterprise(account.provider_id, url, enterprise, url === null);
    if (!firm.ok) return refuse(coreMessage(firm.problem, firm.subject));
    const key = ctx.keys.get(id);
    const twin = core.accounts.find(
      (a) =>
        a.id !== id &&
        key !== undefined &&
        a.provider_id === account.provider_id &&
        sameUrl(ctx.urls.get(a.id) ?? '', url ?? '') &&
        billingProject(a.enterprise) === billingProject(firm.value) &&
        ctx.keys.get(a.id) === key,
    );
    if (twin)
      return refuse(
        `ten klucz w tym projekcie OpenAI jest już kontem „${twin.label}” — wybierz inny projekt`,
      );
    const { enterprise: _previous, ...rest } = account;
    let next: Account = {
      ...rest,
      ...(Object.keys(firm.value).length ? { enterprise: firm.value } : {}),
    };
    if (account.provider_id === AZURE_OPENAI) {
      const models = (firm.value.deployments ?? []).map((d) =>
        fakeModel(d.model ?? d.name, d.name),
      );
      const present = new Set(models.map((m) => m.id));
      const queues = Object.fromEntries(
        Object.entries(account.queues ?? {})
          .map(([task, list]) => [task, list.filter((m) => present.has(m))] as const)
          .filter(([, list]) => list.length > 0),
      );
      const auto_model = suggestFake(models, 'chat', 'balanced')[0] ?? null;
      next = { ...next, models, queues, auto_model };
    }
    core.accounts[index] = next;
    core.emit([{ type: 'AccountChanged', account: next }]);
    return core.reply(next);
  };
}
