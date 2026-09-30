// Drzewo ustawień (PLAN §15), część 2: Sesje i okna … Zaawansowane.
import type { SettingsPageDef } from '../types-system';
import { L, later, number, page, select, text, toggle } from './settings-helpers';

export const SETTINGS_PART_B: readonly SettingsPageDef[] = [
  page('sessions', L('Sesje i okna', 'Sessions and windows'), 1, [
    select(
      'sessions.default_template',
      L('Szablon nowej sesji', 'New session template'),
      L('Od czego zaczyna się nowa rozmowa.', 'What a new conversation starts from.'),
      [
        ['empty', L('Pusty', 'Empty')],
        ['coding', L('Kodowanie', 'Coding')],
        ['research', L('Research', 'Research')],
        ['voice', L('Asystent głosowy', 'Voice assistant')],
        ['admin', L('Administracja PC', 'PC administration')],
      ],
      'empty',
    ),
    toggle(
      'sessions.auto_title',
      L('Automatyczny tytuł', 'Automatic title'),
      L(
        'Tytuł sesji powstaje z pierwszych wiadomości.',
        'The session title is created from the first messages.',
      ),
      true,
    ),
    number(
      'sessions.delete_undo_seconds',
      L('Czas na cofnięcie usunięcia', 'Time to undo deletion'),
      L(
        'Ile sekund możesz cofnąć usunięcie sesji.',
        'How many seconds you can undo deleting a session.',
      ),
      10,
      5,
      30,
      1,
      's',
    ),
    number(
      'ui.max_detached_windows',
      L('Limit odłączonych okien', 'Detached window limit'),
      L(
        'Każde okno zużywa pamięć; powyżej limitu pokażemy ostrzeżenie.',
        'Each window uses memory; above the limit you will see a warning.',
      ),
      4,
      1,
      8,
      1,
      null,
      'machine',
    ),
  ]),
  page('files', L('Pliki', 'Files'), 1, [
    text(
      'files.workdir_root',
      L('Katalog wyjściowy', 'Output folder'),
      L(
        'Tu agentki zapisują pliki, które Ci oddają.',
        'Where agents save the files they hand over to you.',
      ),
      '%USERPROFILE%\\Alfa\\Sesje',
    ),
    toggle(
      'files.image_thumbnails',
      L('Miniatury obrazów', 'Image thumbnails'),
      L(
        'Obrazy w rozmowie jako leniwe miniatury.',
        'Images in the conversation as lazy thumbnails.',
      ),
      true,
    ),
  ]),
  later('logs', L('Logi i prywatność', 'Logs and privacy'), 3, [
    L('Retencja i redakcja', 'Retention and redaction'),
    L('„Co poszło do chmury"', '"What went to the cloud"'),
  ]),
  later('improve', L('Samonaprawa i ulepszanie', 'Self-repair and improvement'), 8, [
    L('Pierścienie zmian R0–R2', 'Change rings R0–R2'),
    L('Kolejka propozycji', 'Proposal queue'),
  ]),
  later('modules', L('Moduły', 'Modules'), 2, [
    L('Lista modułów, włącz / wyłącz', 'Module list, enable / disable'),
    L('Budżety RAM i CPU', 'RAM and CPU budgets'),
  ]),
  page(
    'devices',
    L('Urządzenia', 'Devices'),
    1,
    [
      select(
        'device.battery_mode',
        L('Tryb baterii', 'Battery mode'),
        L(
          'Na baterii Alfa używa lżejszych modeli i rzadziej odświeża wskaźniki.',
          'On battery, Alfa uses lighter models and refreshes indicators less often.',
        ),
        [
          ['auto', L('Automatycznie', 'Automatic')],
          ['always', L('Zawsze oszczędny', 'Always saving')],
          ['never', L('Nigdy', 'Never')],
        ],
        'auto',
        'machine',
      ),
      toggle(
        'device.game_mode',
        L('Tryb gry / pełnego ekranu', 'Game / full-screen mode'),
        L(
          'Przy grze lub filmie na pełnym ekranie włącza „nie przeszkadzać".',
          'Turns on "do not disturb" during full-screen games or videos.',
        ),
        true,
        'machine',
      ),
    ],
    { custom: 'devices' },
  ),
  page('transfer', L('Import i eksport', 'Import and export'), 1, [], { custom: 'transfer' }),
  page('appearance', L('Wygląd', 'Appearance'), 1, [
    select(
      'ui.theme',
      L('Motyw', 'Theme'),
      L('Jasny, ciemny albo za ustawieniem Windows.', 'Light, dark, or following Windows.'),
      [
        ['auto', L('Auto (jak Windows)', 'Auto (like Windows)')],
        ['light', L('Jasny', 'Light')],
        ['dark', L('Ciemny', 'Dark')],
      ],
      'auto',
    ),
    select(
      'ui.accent',
      L('Akcent', 'Accent'),
      L(
        'Kolor mówiącej agentki albo kolor akcentu Windows.',
        'The speaking agent’s colour or the Windows accent colour.',
      ),
      [
        ['agent', L('Kolor agentki', 'Agent colour')],
        ['windows', L('Akcent Windows', 'Windows accent')],
      ],
      'agent',
    ),
    select(
      'ui.density',
      L('Gęstość', 'Density'),
      L('Kompaktowa mieści więcej na ekranie.', 'Compact fits more on screen.'),
      [
        ['comfortable', L('Komfortowa', 'Comfortable')],
        ['compact', L('Kompaktowa', 'Compact')],
      ],
      'comfortable',
    ),
    number(
      'ui.zoom',
      L('Powiększenie', 'Zoom'),
      L(
        'Skala całego interfejsu (Ctrl+= / Ctrl+- / Ctrl+0).',
        'Scale of the whole interface (Ctrl+= / Ctrl+- / Ctrl+0).',
      ),
      100,
      80,
      200,
      10,
      '%',
      'machine',
    ),
    toggle(
      'ui.animations',
      L('Animacje', 'Animations'),
      L(
        'Wyłącz, jeśli ruch Ci przeszkadza. Ustawienie systemowe „ogranicz ruch" też je wyłącza.',
        'Turn off if motion bothers you. The system "reduce motion" setting also disables them.',
      ),
      true,
    ),
    select(
      'ui.column',
      L('Szerokość kolumny rozmowy', 'Conversation column width'),
      L(
        'Wąska (~72 znaki) czyta się najwygodniej; szeroka mieści tabele.',
        'Narrow (~72 characters) reads best; wide fits tables.',
      ),
      [
        ['narrow', L('Wąska', 'Narrow')],
        ['wide', L('Szeroka', 'Wide')],
      ],
      'narrow',
    ),
  ]),
  page(
    'shortcuts',
    L('Skróty', 'Shortcuts'),
    1,
    [
      toggle(
        'composer.enter_sends',
        L('Enter wysyła wiadomość', 'Enter sends the message'),
        L(
          'Wyłączone: Enter to nowa linia, a Ctrl+Enter wysyła.',
          'When off: Enter adds a new line and Ctrl+Enter sends.',
        ),
        true,
      ),
    ],
    { custom: 'shortcuts' },
  ),
  page('notifications', L('Powiadomienia', 'Notifications'), 1, [
    toggle(
      'notify.toasts',
      L('Powiadomienia Windows', 'Windows notifications'),
      L(
        'Gdy okno jest ukryte: koniec zadania, prośba o zatwierdzenie, błąd.',
        'When the window is hidden: task finished, approval request, error.',
      ),
      true,
    ),
    toggle(
      'notify.earcons',
      L('Dźwięki (earcony)', 'Sounds (earcons)'),
      L(
        'Krótkie, ciche dźwięki startu i końca słuchania, błędu i ukończenia.',
        'Short, quiet sounds for listening start/stop, errors and completion.',
      ),
      true,
    ),
    number(
      'notify.earcon_volume',
      L('Głośność dźwięków', 'Sound volume'),
      L('Osobna głośność earconów.', 'Separate volume for earcons.'),
      40,
      0,
      100,
      5,
      '%',
    ),
    toggle(
      'notify.dnd_fullscreen',
      L('Nie przeszkadzać na pełnym ekranie', 'Do not disturb in full screen'),
      L(
        'Automatycznie przy grach i prezentacjach.',
        'Automatically during games and presentations.',
      ),
      true,
    ),
  ]),
  page('language', L('Język', 'Language'), 1, [
    select(
      'ui.locale',
      L('Język interfejsu', 'Interface language'),
      L(
        'Polski albo angielski. Agentki zawsze mówią w rodzaju żeńskim.',
        'Polish or English. Agents always use feminine forms in Polish.',
      ),
      [
        ['pl', L('Polski', 'Polish')],
        ['en', L('English', 'English')],
      ],
      'pl',
    ),
  ]),
  page('updates', L('Aktualizacje', 'Updates'), 1, [
    select(
      'updates.channel',
      L('Kanał aktualizacji', 'Update channel'),
      L(
        'Stabilny dostaje sprawdzone wersje; testowy — wcześniej.',
        'Stable gets tested versions; preview gets them earlier.',
      ),
      [
        ['stable', L('Stabilny', 'Stable')],
        ['preview', L('Testowy', 'Preview')],
      ],
      'stable',
    ),
    toggle(
      'updates.whats_new',
      L('Pokaż „Co nowego"', 'Show "What’s new"'),
      L('Krótka lista zmian po aktualizacji.', 'A short list of changes after updating.'),
      true,
    ),
  ]),
  later('advanced', L('Zaawansowane', 'Advanced'), 8, [
    L('Edytor surowy TOML', 'Raw TOML editor'),
    L('Flagi eksperymentalne', 'Experimental flags'),
  ]),
];
