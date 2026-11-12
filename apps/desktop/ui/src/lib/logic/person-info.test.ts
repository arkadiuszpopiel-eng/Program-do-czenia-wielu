import { describe, expect, it, vi } from 'vitest';
import {
  agents,
  agentIds,
  personInfo,
  personVar,
  personSoftVar,
  personHex,
  type AgentId,
} from '@alfa/ui-kit';
import { openPersonControls } from './person-controls';

describe('persony spoza tokenów nie mogą wyłączyć interfejsu', () => {
  it.each(agentIds)('zachowuje metadane wbudowanej %s', (id) => {
    expect(personInfo(id)).toEqual(agents[id]);
    expect(personVar(id)).toBe(`var(--alfa-agent-${id})`);
  });

  it.each(['luna', '__proto__', 'constructor', 'toString'])(
    'odczytuje ID %s bez dziedziczenia z prototypu',
    (id) => {
      expect(personInfo(id)).toEqual({ id, name: id, glyph: '◇' });
      expect(personVar(id)).toBe('var(--alfa-color-accent)');
      expect(personSoftVar(id)).toBe('var(--alfa-color-surface2)');
      for (const theme of ['light', 'dark'] as const)
        expect(personHex(id, theme)).toMatch(/^#[0-9A-Fa-f]{6}$/);
    },
  );

  it('nie umieszcza nieznanego ID w CSS i ogranicza długość etykiety', () => {
    const id = `bad);url(https://invalid.example/${'x'.repeat(200)})`;
    expect(personVar(id)).not.toContain(id);
    expect(personInfo(id).name.length).toBe(64);
  });

  it.each([...agentIds, 'luna', 'constructor'])('otwiera właściwe ustawienia dla %s', (id) => {
    const app = { selectedAgent: 'alfa' as AgentId, openPanel: vi.fn(), openSettings: vi.fn() };
    openPersonControls(app, id);
    if (agentIds.includes(id as AgentId)) {
      expect(app.openPanel).toHaveBeenCalledWith('agents');
      expect(app.selectedAgent).toBe(id);
      expect(app.openSettings).not.toHaveBeenCalled();
    } else {
      expect(app.openSettings).toHaveBeenCalledWith('builder');
      expect(app.openPanel).not.toHaveBeenCalled();
      expect(app.selectedAgent).toBe('alfa');
    }
  });
});
