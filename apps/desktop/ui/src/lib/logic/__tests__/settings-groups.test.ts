import { describe, expect, it } from 'vitest';
import { FakeAlfaClient } from '../../api/fake/fake-client';
import { SETTINGS_GROUPS, groupSettingsPages } from '../settings-groups';

describe('grupy stron ustawień', () => {
  it('każda strona ze schematu trafia do dokładnie jednej nazwanej grupy', async () => {
    const schema = await new FakeAlfaClient().settings.schema();
    const groups = groupSettingsPages(schema);
    const ids = groups.flatMap((g) => g.pages.map((p) => p.id));
    expect(ids.slice().sort()).toEqual(schema.map((p) => p.id).sort());
    expect(new Set(ids).size).toBe(ids.length);
    expect(groups.map((g) => g.id)).toEqual(SETTINGS_GROUPS.map((g) => g.id));
  });

  it('strona spoza listy nie znika — trafia do grupy „other”, puste grupy są pomijane', () => {
    const groups = groupSettingsPages([{ id: 'voice' }, { id: 'nowa-strona-modulu' }]);
    expect(groups).toEqual([
      { id: 'voice', pages: [{ id: 'voice' }] },
      { id: 'other', pages: [{ id: 'nowa-strona-modulu' }] },
    ]);
  });
});
