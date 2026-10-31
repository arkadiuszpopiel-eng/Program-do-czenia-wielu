import type { SettingDef } from '../types-system';
import { L, select, text, toggle } from './settings-helpers';

export const AGENT_ACCESS_SETTINGS: readonly SettingDef[] = [
  'alfa',
  'beta',
  'gama',
  'delta',
].flatMap((id) => {
  const name = id[0]!.toUpperCase() + id.slice(1);
  const desc = L(
    'Dostęp do narzędzi podlega decyzjom Sędziego Cieni. Zmiana od następnego zadania.',
    'Tool access is subject to the Shadow Judge. Applies to the next task.',
  );
  return [
    select(
      `agents.${id}.access`,
      L(`${name} — Profil dostępu`, `${name} — Access profile`),
      desc,
      [
        ['roles', L('Według ról', 'By role')],
        ['read', L('Tylko odczyt', 'Read only')],
        ['full', L('Pełny dostęp', 'Full access')],
        ['custom', L('Własny', 'Custom')],
        ['off', L('Bez narzędzi', 'No tools')],
      ],
      id === 'alfa' ? 'full' : 'roles',
    ),
    text(`agents.${id}.groups`, L(`${name} — Grupy narzędzi`, `${name} — Tool groups`), desc, ''),
    toggle(
      `agents.${id}.read_only`,
      L(`${name} — Własny profil: tylko odczyt`, `${name} — Custom profile: read only`),
      desc,
      false,
    ),
  ];
});
