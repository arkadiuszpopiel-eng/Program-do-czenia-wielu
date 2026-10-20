// Atrapa: ustawienia dla firm istniejącego konta (`accounts.setEnterprise`) — te same reguły co hub
// kont: sprawdzenie jak przy dodaniu, duplikat klucza w projekcie OpenAI, modele z wdrożeń Azure.
import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import { sentence } from '../fake/api-enterprise';

const KEY = `sk-proj-${'XXXX'.repeat(8)}`;

function setup() {
  return new FakeAlfaClient({ scheduler: new VirtualScheduler() });
}

const openai = (client: FakeAlfaClient, label: string, project: string | null) =>
  client.accounts.add({
    provider_id: 'openai',
    label,
    secret: KEY,
    base_url: null,
    enterprise: project ? { project } : {},
  });

describe('atrapa: ustawienia dla firm istniejącego konta', () => {
  it('OpenAI: organizacja i projekt zapisane, oczyszczone i usuwane; błąd jak w rdzeniu', async () => {
    const client = setup();
    const account = await openai(client, 'Firma', null);
    const saved = await client.accounts.setEnterprise(account.id, {
      organization: ' org-acme1234 ',
      project: 'proj_budget',
    });
    expect(saved.enterprise).toEqual({ organization: 'org-acme1234', project: 'proj_budget' });
    const listed = (await client.accounts.list()).find((a) => a.id === account.id);
    expect(listed?.enterprise).toEqual(saved.enterprise);
    await expect(
      client.accounts.setEnterprise(account.id, { organization: 'acme' }),
    ).rejects.toThrow('Identyfikator organizacji zaczyna się od `org-`, a jest `…`.');
    await expect(
      client.accounts.setEnterprise(account.id, { deployments: [{ name: 'x' }] }),
    ).rejects.toThrow('Wdrożenia i wersja API dotyczą tylko kont Azure OpenAI.');
    const cleared = await client.accounts.setEnterprise(account.id, { organization: '  ' });
    expect(cleared.enterprise).toBeUndefined();
    await expect(client.accounts.setEnterprise('acc-nope', {})).rejects.toThrow(
      'Nie ma takiego konta.',
    );
  });

  it('ten sam klucz w tym samym projekcie to duplikat; inny projekt — wolno', async () => {
    const client = setup();
    await openai(client, 'Budżet A', 'proj_a');
    const b = await openai(client, 'Budżet B', 'proj_b');
    await expect(client.accounts.setEnterprise(b.id, { project: ' proj_a ' })).rejects.toThrow(
      'Ten klucz w tym projekcie OpenAI jest już kontem „Budżet A” — wybierz inny projekt.',
    );
    const moved = await client.accounts.setEnterprise(b.id, { project: 'proj_c' });
    expect(moved.enterprise?.project).toBe('proj_c');
  });

  it('Azure: adres zostaje, modele idą za wdrożeniami, kolejka bez wdrożenia znika', async () => {
    const client = setup();
    const account = await client.accounts.add({
      provider_id: 'azure-openai',
      label: 'Azure',
      secret: KEY,
      base_url: 'https://firma.openai.azure.com',
      enterprise: { deployments: [{ name: 'prod-4o', model: 'gpt-4o' }] },
    });
    expect(account.base_url).toBe('https://firma.openai.azure.com');
    await client.accounts.test(account.id);
    await client.accounts.setQueue(account.id, 'chat', ['gpt-4o']);
    const saved = await client.accounts.setEnterprise(account.id, {
      api_version: '2025-04-01-preview',
      deployments: [{ name: 'prod-mini', model: 'gpt-4o-mini' }],
    });
    expect(saved.base_url).toBe('https://firma.openai.azure.com');
    expect(saved.enterprise?.api_version).toBe('2025-04-01-preview');
    expect(saved.models.map((m) => m.id)).toEqual(['gpt-4o-mini']);
    expect(saved.queues).toEqual({});
    await expect(client.accounts.setEnterprise(account.id, { deployments: [] })).rejects.toThrow(
      'Podaj co najmniej jedno wdrożenie modelu z zasobu Azure OpenAI.',
    );
  });

  it('komunikat huba jako zdanie (jak `app_modules::accounts_enterprise`)', () => {
    expect(sentence('niepoprawne dane: model `x` ma dwa wdrożenia — zostaw jedno')).toBe(
      'Model `x` ma dwa wdrożenia — zostaw jedno.',
    );
    expect(sentence('już.')).toBe('Już.');
  });
});
