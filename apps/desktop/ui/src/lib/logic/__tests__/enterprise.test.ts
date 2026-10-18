// Konta firmowe (C2.12): te same reguły co rdzeń — wspólna tabela przypadków z Rust.
import { describe, expect, it } from 'vitest';
import type { EnterpriseSettings } from '../../api/types-hub';
import {
  AZURE_DEFAULT_API_VERSION,
  checkEnterprise,
  coreMessage,
  emptyEnterpriseForm,
  enterpriseSummary,
  formToSettings,
  isAzureEndpoint,
  maskId,
  parseAzureTarget,
  type EnterpriseProblem,
} from '../enterprise';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
import table from '../../../../../../../crates/accounts-hub-contract/tests/fixtures/enterprise-cases.json';

interface Case {
  readonly provider: string;
  readonly base_url: string | null;
  readonly settings: EnterpriseSettings;
  readonly problem: EnterpriseProblem | null;
  readonly expected?: EnterpriseSettings;
}

const CASES = table.cases as unknown as readonly Case[];

describe('checkEnterprise — wspólna tabela z rdzeniem', () => {
  it('przyjmuje i odrzuca to samo co `EnterpriseSettings::checked`', () => {
    const cases = CASES;
    expect(cases.length).toBeGreaterThanOrEqual(20);
    for (const c of cases) {
      const result = checkEnterprise(c.provider, c.base_url, c.settings);
      if (c.problem === null) {
        expect(result, JSON.stringify(c)).toEqual({ ok: true, value: c.expected });
      } else {
        expect(result.ok, JSON.stringify(c)).toBe(false);
        if (!result.ok) expect(result.problem, JSON.stringify(c)).toBe(c.problem);
      }
    }
  });

  it('komunikat atrapy brzmi jak odmowa rdzenia, a każdy kod ma tłumaczenie', () => {
    expect(coreMessage('duplicate', 'gpt-4o')).toBe(
      'niepoprawne dane: model `gpt-4o` ma dwa wdrożenia — zostaw jedno',
    );
    const codes = new Set(
      CASES.map((c) => c.problem).filter((p): p is NonNullable<typeof p> => p !== null),
    );
    for (const dict of [pl, en] as Record<string, unknown>[]) {
      for (const code of [...codes, 'too_many'])
        expect(dict[`ent.err.${code}`], String(code)).toBeTruthy();
    }
  });
});

describe('formularz kreatora', () => {
  it('Azure zaczyna od jednego pustego wdrożenia, inni dostawcy bez wdrożeń', () => {
    expect(emptyEnterpriseForm('azure-openai').deployments).toHaveLength(1);
    expect(emptyEnterpriseForm('openai').deployments).toHaveLength(0);
  });

  it('puste pola i puste wiersze nie trafiają do rdzenia', () => {
    const settings = formToSettings({
      organization: ' org-A1 ',
      project: '',
      apiVersion: ' ',
      deployments: [
        { name: ' prod ', model: ' gpt-4o ' },
        { name: '', model: '' },
      ],
    });
    expect(settings).toEqual({
      organization: 'org-A1',
      project: null,
      api_version: null,
      deployments: [{ name: 'prod', model: 'gpt-4o' }],
    });
    const azure = checkEnterprise('azure-openai', 'https://c.openai.azure.com', settings);
    expect(azure).toMatchObject({ ok: false, problem: 'org_only_openai' });
  });

  it('domyślna wersja API, gdy pole puste', () => {
    const r = checkEnterprise('azure-openai', 'https://c.openai.azure.com', {
      deployments: [{ name: 'x' }],
    });
    expect(r).toEqual({
      ok: true,
      value: { api_version: AZURE_DEFAULT_API_VERSION, deployments: [{ name: 'x' }] },
    });
  });
});

describe('adres zasobu Azure', () => {
  it('rozpoznaje adres zasobu i odrzuca inne', () => {
    expect(isAzureEndpoint('https://contoso.openai.azure.com')).toBe(true);
    expect(isAzureEndpoint('http://contoso.openai.azure.com')).toBe(false);
    expect(isAzureEndpoint('https://user@contoso.openai.azure.com')).toBe(false);
    expect(isAzureEndpoint('http://[::1]:9000/')).toBe(true);
  });

  it('adres docelowy z portalu dzieli się na zasób, wdrożenie i wersję API', () => {
    expect(
      parseAzureTarget(
        'https://contoso.openai.azure.com/openai/deployments/prod-4o/chat/completions?api-version=2025-04-01-preview',
      ),
    ).toEqual({
      endpoint: 'https://contoso.openai.azure.com',
      deployment: 'prod-4o',
      apiVersion: '2025-04-01-preview',
    });
    expect(parseAzureTarget('https://contoso.openai.azure.com/')).toBeNull();
    expect(parseAzureTarget('to nie adres')).toBeNull();
  });
});

describe('maskowanie i skrót', () => {
  it('identyfikatory bez środka, wdrożenia z modelem', () => {
    expect(maskId('org-AbCdEfGh12345678')).toBe('org-…5678');
    expect(maskId('org-short')).toBe('org-…');
    expect(
      enterpriseSummary({
        organization: 'org-AbCdEfGh12345678',
        project: 'proj_AbCdEfGh1234wxyz',
      }),
    ).toBe('org-…5678 · proj_…wxyz');
    expect(
      enterpriseSummary({
        deployments: [{ name: 'prod-4o', model: 'gpt-4o' }, { name: 'o3-mini' }],
      }),
    ).toBe('gpt-4o → prod-4o · o3-mini');
    expect(enterpriseSummary({})).toBeNull();
    expect(enterpriseSummary(undefined)).toBeNull();
  });
});
