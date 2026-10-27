import { describe, expect, it } from 'vitest';
import {
  blankPipeline,
  blankStage,
  chainLabel,
  copyOf,
  moveStage,
  runMessage,
  validatePipeline,
} from '../pipelines';

describe('pipelines logic', () => {
  it('moves stages within bounds only', () => {
    const a = { ...blankStage(), title: 'A' };
    const b = { ...blankStage(), title: 'B' };
    expect(moveStage([a, b], 0, 1).map((s) => s.title)).toEqual(['B', 'A']);
    expect(moveStage([a, b], 0, -1).map((s) => s.title)).toEqual(['A', 'B']);
    expect(moveStage([a, b], 1, 1).map((s) => s.title)).toEqual(['A', 'B']);
  });

  it('copies under a free name as an own pipeline', () => {
    const def = { ...blankPipeline(), name: 'UI', builtin: true };
    expect(copyOf(def, ['UI']).name).toBe('UI (kopia)');
    expect(copyOf(def, ['UI', 'ui (kopia)']).name).toBe('UI (kopia 2)');
    expect(copyOf(def, []).builtin).toBe(false);
  });

  it('validates name, goal and model format', () => {
    const ok = { ...blankPipeline(), name: 'X' };
    expect(validatePipeline(ok, false)).toBeNull();
    expect(validatePipeline({ ...ok, name: ' ' }, false)).toMatch(/nazwę/);
    expect(validatePipeline({ ...ok, stages: [{ ...ok.stages[0]!, goal: ' ' }] }, false)).toMatch(/Etap 1/);
    expect(
      validatePipeline({ ...ok, stages: [{ ...ok.stages[0]!, model: 'gpt' }] }, true),
    ).toMatch(/provider:model/);
    expect(
      validatePipeline({ ...ok, stages: [{ ...ok.stages[0]!, model: 'openai:gpt-5' }] }, true),
    ).toBeNull();
  });

  it('builds the run message and a chain label', () => {
    expect(runMessage('UI', ' szkic ')).toBe('Uruchom pipeline „UI”.\n\nWejście:\nszkic');
    const def = {
      ...blankPipeline(),
      stages: [{ ...blankStage(), role: 'coder', model: 'openai:gpt-5' }],
    };
    expect(chainLabel(def, false)).toBe('Koderka · gpt-5');
  });
});
