// Opis dostawcy na karcie wyboru: zamiast surowych znaczników katalogu („cn-may-train”, „unknown”)
// zdania zrozumiałe dla użytkownika. Nieznany znacznik nie jest zgadywany — pomijamy go, a gdy
// katalog nie mówi nic, karta podaje to wprost.

/** Fragment opisu: klucz tłumaczenia z parametrami. */
export interface MetaPart {
  readonly key:
    | 'hub.meta.noTrain'
    | 'hub.meta.mayTrain'
    | 'hub.meta.retention'
    | 'hub.meta.law'
    | 'hub.meta.unknown';
  readonly params?: Readonly<Record<string, string | number>>;
}

/** Kody regionów, dla których mamy nazwę (`hub.region.<kod>`). */
export const KNOWN_REGIONS = ['EU', 'EEA', 'US', 'CN', 'SG', 'UK', 'JP'] as const;
export type RegionCode = (typeof KNOWN_REGIONS)[number];

const isKnownRegion = (code: string): code is RegionCode =>
  (KNOWN_REGIONS as readonly string[]).includes(code);

/** Regiony z pola jurysdykcji (`SG|EU`): znane kody i reszta dosłownie; `unknown` → pusto. */
export function regions(jurisdiction: string): readonly (RegionCode | string)[] {
  return jurisdiction
    .split('|')
    .map((code) => code.trim())
    .filter((code) => code !== '' && code.toLowerCase() !== 'unknown')
    .map((code) => (isKnownRegion(code.toUpperCase()) ? code.toUpperCase() : code));
}

/**
 * Fragmenty opisu dostawcy. `regionName` zamienia kod regionu na nazwę w języku interfejsu
 * (nieznany kod zostaje dosłownie).
 */
export function providerMeta(
  provider: { readonly privacy_tag: string; readonly jurisdiction: string },
  regionName: (code: string) => string,
): readonly MetaPart[] {
  const parts: MetaPart[] = [];
  const tag = provider.privacy_tag.toLowerCase();
  if (tag.includes('no-train')) parts.push({ key: 'hub.meta.noTrain' });
  else if (tag.includes('may-train')) parts.push({ key: 'hub.meta.mayTrain' });
  const retention = /retention-(\d+)d/.exec(tag);
  if (retention?.[1])
    parts.push({ key: 'hub.meta.retention', params: { days: Number(retention[1]) } });
  const where = regions(provider.jurisdiction);
  if (where.length > 0) {
    parts.push({ key: 'hub.meta.law', params: { where: where.map(regionName).join(' / ') } });
  }
  return parts.length > 0 ? parts : [{ key: 'hub.meta.unknown' }];
}
