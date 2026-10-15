import { describe, expect, it } from 'vitest';
import { MAX_IMPORT, exportJson, parseCommands, parseTemplates } from '../json-transfer';

describe('json-transfer', () => {
  it('eksport bez identyfikatorów i powrót tym samym kształtem', () => {
    const text = exportJson([{ id: 't1', name: 'Mail', text: 'Napisz do {{kto}}' }]);
    expect(text).not.toContain('t1');
    const back = parseTemplates(text);
    expect(back).toEqual({
      ok: true,
      items: [{ id: '', name: 'Mail', text: 'Napisz do {{kto}}' }],
    });
  });

  it('jeden obiekt zamiast listy; błędy JSON, pustej listy, kształtu i limitu', () => {
    expect(parseTemplates('{"name":"A","text":"B"}')).toMatchObject({ ok: true });
    expect(parseTemplates('{')).toEqual({ ok: false, error: { kind: 'json' } });
    expect(parseTemplates('[]')).toEqual({ ok: false, error: { kind: 'empty' } });
    expect(parseTemplates('[{"name":"A"}]')).toMatchObject({
      ok: false,
      error: { kind: 'shape', index: 1 },
    });
    expect(parseTemplates('[1]')).toEqual({ ok: false, error: { kind: 'notObject', index: 1 } });
    const many = JSON.stringify(
      Array.from({ length: MAX_IMPORT + 1 }, () => ({ name: 'a', text: 'b' })),
    );
    expect(parseTemplates(many)).toEqual({
      ok: false,
      error: { kind: 'tooMany', max: MAX_IMPORT },
    });
  });

  it('polecenia: domyślny tryb, włączenie i priorytet w zakresie', () => {
    const r = parseCommands(
      '[{"name":"Dobranoc","phrases":["dobranoc",""],"reply":"Śpij"},{"name":"X","phrases":["x"],"reply":"y","mode":"regex","enabled":false,"priority":99}]',
    );
    expect(r).toEqual({
      ok: true,
      items: [
        {
          id: '',
          name: 'Dobranoc',
          phrases: ['dobranoc'],
          mode: 'exact',
          reply: 'Śpij',
          enabled: true,
          priority: 0,
        },
        {
          id: '',
          name: 'X',
          phrases: ['x'],
          mode: 'regex',
          reply: 'y',
          enabled: false,
          priority: 10,
        },
      ],
    });
    expect(parseCommands('{"name":"A","phrases":["a"],"reply":"b","mode":"fuzzy"}')).toMatchObject({
      ok: false,
    });
    expect(parseCommands('{"name":"A","phrases":[],"reply":"b"}')).toMatchObject({ ok: false });
  });
});
