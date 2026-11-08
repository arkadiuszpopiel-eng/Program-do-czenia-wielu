// Panele chowane (właściciel 27.10; domyślnie włączone od 29.10) i uproszczony ekran (K.3) —
// wydzielone z `settings-part-b.ts` (limit rozmiaru).
import { L, toggle } from './settings-helpers';

export const PANELS_AUTOHIDE = toggle(
  'ui.panels_autohide',
  L('Panele chowają się same', 'Panels hide themselves'),
  L(
    'Panel Sesje i panel prawy chowają się, a przy krawędzi okna świeci znacznik — najedź na niego, a panel wysunie się nad rozmowę. „Przypnij panel” w wysuniętym panelu przywraca stały układ.',
    'The Sessions panel and the side panel hide, and a marker glows at the window edge — hover it and the panel slides out over the conversation. “Pin panel” in the slid-out panel restores the fixed layout.',
  ),
  true,
);

export const SIMPLE_SCREEN = toggle(
  'ui.simple',
  L('Uproszczony ekran', 'Simplified screen'),
  L(
    'Mniej przycisków: chowa przełączniki paneli, Pipeline’y, wybór agentki i rzadsze akcje wiadomości (zostają „Kopiuj” i „Przeczytaj na głos”). Wskaźniki mikrofonu, kamery i sterowania oraz tryb pracy są widoczne zawsze. To wygoda, nie blokada — ochronę dziecka da konto Dziecko.',
    'Fewer buttons: hides the panel toggles, Pipelines, the agent picker and less common message actions (“Copy” and “Read aloud” stay). The microphone, camera and control indicators and the work mode are always visible. A convenience, not a lock — a Child account will protect children.',
  ),
  false,
);
