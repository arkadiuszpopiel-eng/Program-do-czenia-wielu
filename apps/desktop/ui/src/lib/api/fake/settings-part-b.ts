// Drzewo ustawień (PLAN §15), część 2: Sesje i okna i dalej (Zaawansowane — w `settings-schema.ts`).
import type { SettingsPageDef } from '../types-system';
import { L, later, number, page, select, text, toggle } from './settings-helpers';
import { PANELS_AUTOHIDE } from './settings-panels';

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
        'Obrazy w rozmowie pokazujemy jako miniatury, wczytywane dopiero wtedy, gdy są widoczne.',
        'Images in the conversation are shown as thumbnails, loaded only once they are visible.',
      ),
      true,
    ),
  ]),
  later('logs', L('Logi i prywatność', 'Logs and privacy'), 3, [
    L('Retencja i redakcja', 'Retention and redaction'),
    L('„Co poszło do chmury"', '"What went to the cloud"'),
  ]),
  page(
    'improve',
    L('Zdrowie systemu', 'System health'),
    1,
    [
      select(
        'diagnostician.autonomy',
        L('Samodzielność Diagnosty', 'Diagnostician autonomy'),
        L(
          'Co Diagnosta naprawia sam (z „Cofnij”), a co tylko proponuje. Naprawy Jądra zawsze zatwierdzasz w oknie Brokera. Zmiana działa od następnego uruchomienia.',
          'What the Diagnostician fixes on its own (with Undo) and what it only proposes. Kernel repairs always need approval in the Broker window. Takes effect after the next start.',
        ),
        [
          ['propose_only', L('Tylko proponuje', 'Propose only')],
          ['auto_low_risk', L('Sam naprawia niskie ryzyko', 'Fixes low risk on its own')],
          [
            'auto_medium_risk',
            L('Sam naprawia niskie i średnie ryzyko', 'Fixes low and medium risk on its own'),
          ],
        ],
        'auto_low_risk',
      ),
    ],
    { custom: 'health' },
  ),
  page('modules', L('Moduły', 'Modules'), 1, [], { custom: 'modules' }),
  page(
    'cameras',
    L('Kamery', 'Cameras'),
    1,
    [
      toggle(
        'camera.enabled',
        L('Kamery włączone', 'Cameras enabled'),
        L(
          'Przełącznik główny. Wyłączony: żadna kamera się nie otworzy, niezależnie od tego, kto prosi o obraz. Włączony: kamera otwiera się na żądanie (podgląd) i sama zasypia; działającą kamerę zawsze widać w górnej belce.',
          'Master switch. Off: no camera opens, no matter who asks for an image. On: a camera opens on request (preview) and goes to sleep by itself; a live camera is always shown in the top bar.',
        ),
        false,
      ),
      toggle(
        'camera.auto_look',
        L('„Opisz, co widzisz” robi zdjęcie', '“Describe what you see” takes a photo'),
        L(
          'Gdy napiszesz „opisz mi to, co widzisz” (albo „co widzisz na kamerze Kuchnia?”), Alfa dołączy do wiadomości zdjęcie z kamery — nazwanej w prośbie albo pierwszej włączonej. Działa tylko przy włączonych kamerach; zdjęcie zostaje w rozmowie jak każdy załącznik.',
          'When you write “describe what you see” (or “what do you see on camera Kitchen?”), Alfa attaches a photo from the camera — the one named in the request or the first enabled one. Works only while cameras are enabled; the photo stays in the conversation like any attachment.',
        ),
        true,
      ),
    ],
    { custom: 'cameras' },
  ),
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
      'ui.style',
      L('Styl', 'Style'),
      L(
        'Wygląd całego interfejsu. Każdy styl ma wersję jasną i ciemną — wybiera ją „Motyw”.',
        'The look of the whole interface. Every style has a light and a dark version — picked by “Theme”.',
      ),
      [
        ['classic', L('Klasyczny', 'Classic')],
        ['glass', L('Szkło', 'Glass')],
        ['next', L('Next', 'Next')],
      ],
      'classic',
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
      'ui.zoom_fit',
      L('Dopasuj skalę do okna', 'Fit scale to window'),
      L(
        'Na szerokich oknach (ponad 1600 px) interfejs rośnie razem z oknem — do 125 % przy 2560 px i 150 % przy 3840 px. Powiększenie wyżej działa dodatkowo.',
        'On wide windows (over 1600 px) the interface grows with the window — up to 125% at 2560 px and 150% at 3840 px. The zoom above applies on top.',
      ),
      true,
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
        'Dopasowana rośnie z oknem (do ok. 110 znaków); wąska (~72 znaki) czyta się najwygodniej; szeroka mieści tabele.',
        'Fitted grows with the window (up to about 110 characters); narrow (~72 characters) reads best; wide fits tables.',
      ),
      [
        ['auto', L('Dopasowana do okna', 'Fitted to window')],
        ['narrow', L('Wąska', 'Narrow')],
        ['wide', L('Szeroka', 'Wide')],
      ],
      'auto',
    ),
    PANELS_AUTOHIDE,
    select(
      'ui.avatars',
      L('Awatary agentek', 'Agent avatars'),
      L(
        '„Żywe” — agentka, która mówi albo pracuje, dostaje twarz: mruga, a mówiąc rusza ustami w rytm mowy. „Same litery” — zostaje litera z pulsującym pierścieniem.',
        '“Animated” — an agent that is speaking or working gets a face: it blinks and moves its mouth in time with speech. “Letters only” keeps the letter with a pulsing ring.',
      ),
      [
        ['animated', L('Żywe (zalecane)', 'Animated (recommended)')],
        ['letters', L('Same litery', 'Letters only')],
      ],
      'animated',
    ),
    toggle(
      'ui.reply_details',
      L('Szczegóły pod odpowiedzią', 'Details under replies'),
      L(
        'Pod każdą odpowiedzią: model, liczba tokenów, koszt i czas. Wyłącz, jeśli wolisz czystą rozmowę — te dane zostają w osi czasu.',
        'Under each reply: model, token counts, cost and time. Turn off for a cleaner conversation — the data stays in the timeline.',
      ),
      true,
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
      L('Dźwięki sygnałowe', 'Signal sounds'),
      L(
        'Krótkie, ciche dźwięki startu i końca słuchania, błędu i ukończenia.',
        'Short, quiet sounds for listening start/stop, errors and completion.',
      ),
      true,
    ),
    number(
      'notify.earcon_volume',
      L('Głośność dźwięków', 'Sound volume'),
      L(
        'Głośność dźwięków sygnałowych, niezależna od głosu agentek.',
        'Volume of signal sounds, independent of the agents’ voices.',
      ),
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
  page(
    'updates',
    L('Aktualizacje', 'Updates'),
    1,
    [
      select(
        'updates.channel',
        L('Kanał aktualizacji', 'Update channel'),
        L(
          'Stabilny dostaje sprawdzone wersje; testowy (beta) — wcześniej, także wersje przedpremierowe.',
          'Stable gets tested versions; preview (beta) gets them earlier, including pre-releases.',
        ),
        [
          ['stable', L('Stabilny', 'Stable')],
          ['preview', L('Testowy (beta)', 'Preview (beta)')],
        ],
        'stable',
      ),
      select(
        'updates.mode',
        L('Instalowanie aktualizacji', 'Installing updates'),
        L(
          'Automatycznie: Alfa sprawdza raz dziennie, pobiera i przygotowuje nową wersję (działa od ponownego uruchomienia). Pytaj: sprawdza raz dziennie i pyta przed pobraniem. Ręcznie: tylko po „Sprawdź teraz”. Bez telemetrii — tylko pobranie listy wydań.',
          'Automatic: Alfa checks daily, downloads and prepares the new version (active after restart). Ask: checks daily and asks before downloading. Manual: only on “Check now”. No telemetry — only the release list is fetched.',
        ),
        [
          ['auto', L('Automatycznie', 'Automatically')],
          ['ask', L('Pytaj przed pobraniem', 'Ask before downloading')],
          ['manual', L('Ręcznie', 'Manually')],
        ],
        'ask',
      ),
      toggle(
        'updates.whats_new',
        L('Pokaż „Co nowego"', 'Show "What’s new"'),
        L('Krótka lista zmian po aktualizacji.', 'A short list of changes after updating.'),
        true,
      ),
    ],
    { custom: 'updates' },
  ),
  page('about', L('O programie', 'About'), 1, [], { custom: 'about' }),
];
