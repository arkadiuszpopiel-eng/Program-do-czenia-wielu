// Konta firmowe (C2.12): organizacja i projekt OpenAI, zasób Azure OpenAI z wdrożeniami. Te same
// reguły co rdzeń (`crates/accounts-hub-contract/src/enterprise.rs`, wspólna tabela przypadków
// `tests/fixtures/enterprise-cases.json`): kreator blokuje zapis z czytelnym powodem, a atrapa
// odrzuca dokładnie to, co odrzuciłby rdzeń.
import type { EnterpriseSettings } from '../api/types-hub';

export const OPENAI = 'openai';
export const AZURE_OPENAI = 'azure-openai';
/** Wersja API Azure OpenAI, gdy konto jej nie podaje (jak `AZURE_DEFAULT_API_VERSION` rdzenia). */
export const AZURE_DEFAULT_API_VERSION = '2024-10-21';
export const MAX_DEPLOYMENTS = 32;
const AZURE_HOSTS = ['.openai.azure.com', '.cognitiveservices.azure.com'] as const;

/** Formularz kreatora: puste pole = brak wartości. */
export interface EnterpriseForm {
  organization: string;
  project: string;
  apiVersion: string;
  deployments: { name: string; model: string }[];
}

export const emptyEnterpriseForm = (providerId = ''): EnterpriseForm => ({
  organization: '',
  project: '',
  apiVersion: '',
  // Azure bez wdrożenia nie działa — od razu jeden wiersz do wypełnienia.
  deployments: providerId === AZURE_OPENAI ? [{ name: '', model: '' }] : [],
});

export type EnterpriseProblem =
  | 'org_only_openai'
  | 'org_format'
  | 'project_format'
  | 'azure_only'
  | 'endpoint'
  | 'api_version'
  | 'no_deployments'
  | 'too_many'
  | 'deployment_name'
  | 'model_name'
  | 'duplicate';

export type EnterpriseCheck =
  | { readonly ok: true; readonly value: EnterpriseSettings }
  | { readonly ok: false; readonly problem: EnterpriseProblem; readonly subject: string };

const plain = (id: string, extra = '') =>
  id.length > 0 &&
  id.length <= 64 &&
  [...id].every((c) => /[A-Za-z0-9_-]/.test(c) || extra.includes(c));

export const isOrganization = (id: string): boolean =>
  id.startsWith('org-') && id.length > 4 && plain(id);
export const isProject = (id: string): boolean => /^proj[_-]./.test(id) && plain(id);
/** Projekt OpenAI konta (pusty = brak); ten sam klucz w innym projekcie to osobne konto. */
export const billingProject = (e?: EnterpriseSettings | null): string | null =>
  e?.project?.trim() || null;
export const isApiVersion = (v: string): boolean => /^\d{4}-\d{2}-\d{2}(-preview)?$/.test(v);
export const isDeploymentName = (n: string): boolean => plain(n, '.') && !n.startsWith('.');

/** Identyfikator do pokazania w komunikacie: przedrostek, wielokropek, cztery ostatnie znaki. */
export function maskId(id: string): string {
  const cut = id.search(/[-_]/);
  const prefix = cut >= 0 ? id.slice(0, cut + 1) : '';
  const chars = [...id];
  if (chars.length <= prefix.length + 8) return `${prefix}…`;
  return `${prefix}…${chars.slice(-4).join('')}`;
}

/** Adres zasobu: `https://<zasób>.openai.azure.com` bez ścieżki (host lokalny przez http też). */
export function isAzureEndpoint(url: string): boolean {
  const lower = url.trim().toLowerCase();
  const match = /^(https?):\/\/([^/?#]*)(.*)$/.exec(lower);
  if (!match) return false;
  const [, scheme, authority = '', rest = ''] = match;
  if (!authority || /[@\\\s]/.test(authority) || !['', '/'].includes(rest)) return false;
  const host = authority.startsWith('[') ? `${authority.split(']')[0]}]` : authority.split(':')[0];
  const local = host === 'localhost' || host === '127.0.0.1' || host === '[::1]';
  if (local) return true;
  if (scheme !== 'https' || authority.includes(':')) return false;
  return AZURE_HOSTS.some((suffix) => {
    const resource = host?.endsWith(suffix) ? host.slice(0, -suffix.length) : '';
    return /^[a-z0-9-]+$/.test(resource);
  });
}

/** Adres docelowy wklejony z portalu Azure → zasób, wdrożenie i wersja API (gdy są w adresie). */
export function parseAzureTarget(
  url: string,
): { endpoint: string; deployment: string | null; apiVersion: string | null } | null {
  let parsed: URL;
  try {
    parsed = new URL(url.trim());
  } catch {
    return null;
  }
  const deployment = /^\/openai\/deployments\/([^/]+)/.exec(parsed.pathname)?.[1] ?? null;
  const apiVersion = parsed.searchParams.get('api-version');
  if (!deployment && !apiVersion && ['', '/'].includes(parsed.pathname)) return null;
  return {
    endpoint: `${parsed.protocol}//${parsed.host}`,
    deployment: deployment ? decodeURIComponent(deployment) : null,
    apiVersion,
  };
}

const fail = (problem: EnterpriseProblem, subject = ''): EnterpriseCheck => ({
  ok: false,
  problem,
  subject,
});

/** Ustawienia z formularza (przycięte, puste pola pominięte). */
export function formToSettings(form: EnterpriseForm): EnterpriseSettings {
  const text = (v: string) => v.trim() || null;
  return {
    organization: text(form.organization),
    project: text(form.project),
    api_version: text(form.apiVersion),
    deployments: form.deployments
      .filter((d) => d.name.trim() || d.model.trim())
      .map((d) => ({ name: d.name.trim(), model: text(d.model) })),
  };
}

/** Oczyszczenie i sprawdzenie jak `EnterpriseSettings::checked` w rdzeniu. */
export function checkEnterprise(
  providerId: string,
  baseUrl: string | null,
  settings: EnterpriseSettings | undefined,
): EnterpriseCheck {
  const text = (v: string | null | undefined) => v?.trim() || null;
  const organization = text(settings?.organization);
  const project = text(settings?.project);
  const deployments = (settings?.deployments ?? []).map((d) => ({
    name: d.name.trim(),
    model: text(d.model),
  }));
  let apiVersion = text(settings?.api_version);
  if ((organization || project) && providerId !== OPENAI) return fail('org_only_openai');
  if (organization && !isOrganization(organization))
    return fail('org_format', maskId(organization));
  if (project && !isProject(project)) return fail('project_format', maskId(project));
  const clean = (rest: Partial<EnterpriseSettings>): EnterpriseCheck => ({
    ok: true,
    value: {
      ...(organization ? { organization } : {}),
      ...(project ? { project } : {}),
      ...rest,
    },
  });
  if (providerId !== AZURE_OPENAI) {
    return apiVersion || deployments.length ? fail('azure_only') : clean({});
  }
  if (!isAzureEndpoint(baseUrl ?? '')) return fail('endpoint', baseUrl ?? '');
  apiVersion ??= AZURE_DEFAULT_API_VERSION;
  if (!isApiVersion(apiVersion)) return fail('api_version', apiVersion);
  if (!deployments.length) return fail('no_deployments');
  if (deployments.length > MAX_DEPLOYMENTS) return fail('too_many', String(MAX_DEPLOYMENTS));
  const seen = new Set<string>();
  for (const d of deployments) {
    if (!isDeploymentName(d.name)) return fail('deployment_name', d.name);
    if (d.model && !plain(d.model, '.:')) return fail('model_name', d.name);
    const alias = d.model ?? d.name;
    if (seen.has(alias)) return fail('duplicate', alias);
    seen.add(alias);
  }
  const alias = (d: { name: string; model: string | null }) => d.model ?? d.name;
  const sorted = [...deployments].sort((a, b) => (alias(a) < alias(b) ? -1 : 1));
  return clean({
    api_version: apiVersion,
    deployments: sorted.map((d) => (d.model ? { name: d.name, model: d.model } : { name: d.name })),
  });
}

/** Odmowa tak, jak sformułowałby ją rdzeń (atrapa API, komunikaty po polsku jak w rdzeniu). */
export function coreMessage(problem: EnterpriseProblem, subject: string): string {
  const messages: Record<EnterpriseProblem, string> = {
    org_only_openai: 'organizacja i projekt dotyczą tylko kont OpenAI',
    org_format: `identyfikator organizacji zaczyna się od \`org-\`, a jest \`${subject}\``,
    project_format: `identyfikator projektu zaczyna się od \`proj_\`, a jest \`${subject}\``,
    azure_only: 'wdrożenia i wersja API dotyczą tylko kont Azure OpenAI',
    endpoint: `adres zasobu Azure OpenAI ma postać https://<zasób>.openai.azure.com (bez dalszej ścieżki), a jest \`${subject}\``,
    api_version: `wersja API Azure ma postać RRRR-MM-DD (np. ${AZURE_DEFAULT_API_VERSION}), a jest \`${subject}\``,
    no_deployments: 'podaj co najmniej jedno wdrożenie modelu z zasobu Azure OpenAI',
    too_many: `najwyżej ${MAX_DEPLOYMENTS} wdrożeń w jednym koncie`,
    deployment_name: `nazwa wdrożenia \`${subject}\` może mieć tylko litery, cyfry, \`-\`, \`_\` i \`.\``,
    model_name: `nazwa modelu wdrożenia \`${subject}\` może mieć tylko litery, cyfry, \`-\`, \`_\`, \`.\` i \`:\``,
    duplicate: `model \`${subject}\` ma dwa wdrożenia — zostaw jedno`,
  };
  return `niepoprawne dane: ${messages[problem]}`;
}

/** Skrót ustawień do listy kont (bez pełnych identyfikatorów). */
export function enterpriseSummary(settings: EnterpriseSettings | undefined): string | null {
  if (!settings) return null;
  const parts = [
    settings.organization ? maskId(settings.organization) : '',
    settings.project ? maskId(settings.project) : '',
    ...(settings.deployments ?? []).map((d) =>
      d.model && d.model !== d.name ? `${d.model} → ${d.name}` : d.name,
    ),
  ].filter(Boolean);
  return parts.length ? parts.join(' · ') : null;
}
