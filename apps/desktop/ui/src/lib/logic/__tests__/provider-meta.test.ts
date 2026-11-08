import { describe, expect, it } from 'vitest';
import { providerMeta, regions } from '../provider-meta';

const name = (code: string) => ({ EU: 'UE', CN: 'Chiny', SG: 'Singapur' })[code] ?? code;

describe('opis dostawcy na karcie wyboru', () => {
  it('nie pokazuje surowego „unknown” — mówi, że zasady są nieopisane', () => {
    expect(providerMeta({ privacy_tag: 'unknown', jurisdiction: 'unknown' }, name)).toEqual([
      { key: 'hub.meta.unknown' },
    ]);
  });

  it('rozpoznaje trenowanie na danych i jurysdykcję', () => {
    expect(providerMeta({ privacy_tag: 'cn-may-train', jurisdiction: 'CN' }, name)).toEqual([
      { key: 'hub.meta.mayTrain' },
      { key: 'hub.meta.law', params: { where: 'Chiny' } },
    ]);
    expect(
      providerMeta({ privacy_tag: 'google-paid-eea-no-train', jurisdiction: 'unknown' }, name),
    ).toEqual([{ key: 'hub.meta.noTrain' }]);
  });

  it('podaje czas przechowywania i kilka regionów', () => {
    expect(providerMeta({ privacy_tag: 'xai-retention-30d', jurisdiction: 'SG|EU' }, name)).toEqual(
      [
        { key: 'hub.meta.retention', params: { days: 30 } },
        { key: 'hub.meta.law', params: { where: 'Singapur / UE' } },
      ],
    );
  });

  it('nieznany kod regionu zostaje dosłownie, pusty i „unknown” znikają', () => {
    expect(regions('eu| |XX|unknown')).toEqual(['EU', 'XX']);
    expect(providerMeta({ privacy_tag: 'sg', jurisdiction: 'XX' }, name)).toEqual([
      { key: 'hub.meta.law', params: { where: 'XX' } },
    ]);
  });
});
