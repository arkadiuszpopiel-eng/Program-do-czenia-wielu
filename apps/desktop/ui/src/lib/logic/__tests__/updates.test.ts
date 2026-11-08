import { describe, expect, it } from 'vitest';
import type { UpdatesView } from '../../api/types-updates';
import {
  compareVersions,
  filterLicenses,
  isRollback,
  phaseMessage,
  progressPercent,
  updateActions,
} from '../updates';

const base: UpdatesView = {
  phase: 'idle',
  current: '1.0.0',
  channel: 'stable',
  mode: 'ask',
  available: null,
  progress: null,
  ready: null,
  previous: null,
  last_check: null,
  error: null,
  restart_blocked: null,
};

describe('aktualizacje — logika widoku', () => {
  it('porównuje wersje semver z przedpremierowymi', () => {
    expect(compareVersions('1.2.0', '1.1.9')).toBe(1);
    expect(compareVersions('1.2.0', '1.2.0-beta.1')).toBe(1);
    expect(compareVersions('1.2.0-beta.2', '1.2.0-beta.10')).toBe(-1);
    expect(compareVersions('1.2.0-alpha', '1.2.0-beta')).toBe(-1);
    expect(compareVersions('1.2.0-1', '1.2.0-beta')).toBe(-1);
    expect(compareVersions('1.2.0-beta', '1.2.0-beta.1')).toBe(-1);
    expect(compareVersions('2.0.0', '2.0.0')).toBe(0);
  });

  it('rozpoznaje przywrócenie starszej wersji i komunikat etapu', () => {
    const update = { ...base, phase: 'ready' as const, ready: '1.1.0', previous: '1.0.0' };
    expect(isRollback(update)).toBe(false);
    expect(phaseMessage(update)).toEqual({
      key: 'updates.phase.ready',
      params: { version: '1.1.0' },
    });
    const back = { ...base, phase: 'ready' as const, ready: '0.9.0' };
    expect(isRollback(back)).toBe(true);
    expect(phaseMessage(back).key).toBe('updates.phase.readyRollback');
    const available = {
      ...base,
      phase: 'available' as const,
      available: { version: '1.2.0', notes: '' },
    };
    expect(phaseMessage(available).params.version).toBe('1.2.0');
  });

  it('liczy postęp i dostępne akcje', () => {
    expect(progressPercent(base)).toBeNull();
    const downloading: UpdatesView = {
      ...base,
      phase: 'downloading',
      available: { version: '1.1.0', notes: '' },
      progress: { downloaded: 25, total: 100, resumed: false },
    };
    expect(progressPercent(downloading)).toBe(25);
    expect(
      progressPercent({ ...downloading, progress: { downloaded: 5, total: null, resumed: true } }),
    ).toBeNull();
    const busy = updateActions(downloading);
    expect(busy).toMatchObject({ check: false, cancel: true, download: false, rollback: false });
    const failed = updateActions({ ...downloading, phase: 'failed' });
    expect(failed).toMatchObject({ resume: true, download: false, check: true });
    const fresh = updateActions({ ...downloading, phase: 'available', progress: null });
    expect(fresh).toMatchObject({ download: true, resume: false });
    const ready = updateActions({ ...base, phase: 'ready', ready: '1.1.0', previous: '1.0.0' });
    expect(ready).toMatchObject({ restart: true, rollback: false });
    expect(updateActions({ ...base, previous: '0.9.0' }).rollback).toBe(true);
    expect(updateActions({ ...base, phase: 'disabled' }).check).toBe(false);
  });

  it('filtruje licencje po nazwie i licencji', () => {
    const list = [
      { name: 'tauri', version: '2.12.0', license: 'MIT OR Apache-2.0', source: 'cargo' as const },
      { name: 'svelte', version: '5.57.1', license: 'MIT', source: 'npm' as const },
    ];
    expect(filterLicenses(list, '')).toHaveLength(2);
    expect(filterLicenses(list, 'APACHE')).toEqual([list[0]]);
    expect(filterLicenses(list, 'svel')).toEqual([list[1]]);
  });
});
