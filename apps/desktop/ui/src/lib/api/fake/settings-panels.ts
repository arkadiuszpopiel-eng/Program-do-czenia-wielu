// Panele chowane (właściciel 27.10) — wydzielone z `settings-part-b.ts` (limit rozmiaru).
import { L, toggle } from './settings-helpers';

export const PANELS_AUTOHIDE = toggle(
  'ui.panels_autohide',
  L('Panele chowają się same', 'Panels hide themselves'),
  L(
    'Panel Sesje i panel prawy chowają się, a przy krawędzi okna świeci znacznik — najedź na niego, a panel wysunie się nad rozmowę. „Przypnij panel” w wysuniętym panelu przywraca stały układ.',
    'The Sessions panel and the side panel hide, and a marker glows at the window edge — hover it and the panel slides out over the conversation. “Pin panel” in the slid-out panel restores the fixed layout.',
  ),
  false,
);
