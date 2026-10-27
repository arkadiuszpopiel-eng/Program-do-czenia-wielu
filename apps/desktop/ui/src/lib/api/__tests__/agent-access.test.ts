import { describe, expect, it } from 'vitest';
import { SETTINGS_SCHEMA } from '../fake/settings-schema';

describe('ustawienia wykonania agentek', () => {
  it('udostępnia limit 500 i odrębny profil dostępu dla każdej agentki', () => {
    const defs = SETTINGS_SCHEMA.flatMap((p) => p.settings);
    expect(defs.find((d) => d.key === 'agents.max_steps')?.control).toMatchObject({ max: 500 });
    for (const id of ['alfa', 'beta', 'gama', 'delta']) {
      expect(defs.find((d) => d.key === `agents.${id}.access`)?.default).toBe(
        id === 'alfa' ? 'full' : 'roles',
      );
      expect(defs.find((d) => d.key === `agents.${id}.groups`)).toBeDefined();
      expect(defs.find((d) => d.key === `agents.${id}.read_only`)).toBeDefined();
    }
  });
});
