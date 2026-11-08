import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../../api/fake/fake-client';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
import {
  QUICK_TESTS,
  change,
  diskKeys,
  fmtValue,
  gb,
  keyOf,
  mainMetric,
  testLabelKey,
  trend,
  uptime,
  verdictClass,
} from '../diag';

const client = () => new FakeAlfaClient({ scheduler: new VirtualScheduler() });

describe('logika diagnostyki', () => {
  it('klucze, etykiety i formatowanie', () => {
    expect(keyOf({ test: 'disk', target: 'E:' })).toBe('disk:E:');
    expect(keyOf({ test: 'memory' })).toBe('memory');
    expect(diskKeys(['memory', 'disk:C:', 'disk:E:'])).toEqual(['disk:C:', 'disk:E:']);
    expect(testLabelKey('disk:E:')).toEqual({ key: 'diag.test.disk', letter: 'E:' });
    for (const k of QUICK_TESTS) {
      expect(pl[`diag.test.${k}` as keyof typeof pl]).toBeTruthy();
      expect(en[`diag.test.${k}` as keyof typeof en]).toBeTruthy();
    }
    expect(verdictClass('fail')).toBe('wk-error');
    expect(fmtValue({ value: 1450, unit: 'MB/s' })).toBe('1450 MB/s');
    expect(fmtValue({ value: 0.123, unit: 'ms' })).toBe('0,12 ms');
    expect(gb(2048)).toBe('2 GB');
    expect(uptime(3 * 3600 + 120)).toBe('3 h 2 min');
    expect(mainMetric('disk:C:')).toBe('seq_write_mbps');
    expect(mainMetric('integrity')).toBeNull();
  });

  it('trend z przebiegów (najnowsze pierwsze) i zmiana procentowa', async () => {
    const c = client();
    await c.diag.run(['disk:C:']);
    await c.diag.run(['disk:C:', 'disk:E:']);
    const runs = await c.diag.runs(10);
    expect(runs).toHaveLength(2);
    const values = trend(runs, 'disk:C:', 'seq_write_mbps');
    expect(values).toEqual([1450, 1450]);
    expect(change(values)).toBe(0);
    expect(change([100, 60])).toBe(-40);
    expect(change([5])).toBeNull();
  });
});

describe('atrapa diagnostyki', () => {
  it('stan, zapis, testy, błędny klucz, raport', async () => {
    const c = client();
    const v = await c.diag.state();
    expect(v.latest?.disks).toHaveLength(2);
    expect(v.integrity?.verdict).toBe('fail');
    expect(v.available).toContain('disk:E:');
    const run = await c.diag.run(['integrity', 'cpu_single', 'disk:E:']);
    expect(run.overall).toBe('fail');
    expect(run.results[2]?.verdict).toBe('warn');
    await expect(c.diag.run(['zly'])).rejects.toThrow(/Nieznany test/);
    await expect(c.diag.run([])).rejects.toThrow();
    expect((await c.diag.state()).runs).toBe(1);
    expect(await c.diag.export()).toMatch(/Diagnostyka.*\.md$/);
  });
});
