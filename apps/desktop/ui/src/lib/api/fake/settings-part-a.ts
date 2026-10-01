// Drzewo ustawień (PLAN §15), część 1: Ogólne … Pamięć.
import type { SettingsPageDef } from '../types-system';
import { L, later, number, page, select, toggle } from './settings-helpers';

export const SETTINGS_PART_A: readonly SettingsPageDef[] = [
  page('general', L('Ogólne', 'General'), 1, [
    toggle(
      'general.start_with_system',
      L('Uruchamiaj z systemem', 'Start with Windows'),
      L('Alfa startuje do zasobnika po zalogowaniu.', 'Alfa starts in the tray after you sign in.'),
      true,
    ),
    toggle(
      'general.close_to_tray',
      L('Zamknięcie okna chowa do zasobnika', 'Closing the window hides it to the tray'),
      L(
        'Okno znika, ale Alfa dalej słucha i pracuje w tle.',
        'The window disappears, but Alfa keeps listening and working.',
      ),
      true,
    ),
    number(
      'general.destroy_webview_after',
      L('Zwolnij pamięć okna po', 'Free window memory after'),
      L(
        'Po tylu minutach ukrycia widok okna jest zamykany, żeby oszczędzać RAM.',
        'After this many minutes hidden, the window view is closed to save RAM.',
      ),
      10,
      1,
      120,
      1,
      'min',
      'machine',
    ),
    select(
      'general.view_profile',
      L('Profil widoku ustawień', 'Settings view profile'),
      L(
        'Ile opcji pokazujemy: Prosty, Zaawansowany albo Ekspert.',
        'How many options to show: Simple, Advanced or Expert.',
      ),
      [
        ['simple', L('Prosty', 'Simple')],
        ['advanced', L('Zaawansowany', 'Advanced')],
        ['expert', L('Ekspert', 'Expert')],
      ],
      'simple',
    ),
    select(
      'quick.session_mode',
      L('Szybkie pytanie: sesja', 'Quick ask: session'),
      L(
        'Czy pytania trafiają do jednej sesji „Szybkie pytania", czy każde tworzy nową.',
        'Whether questions go to a single "Quick questions" session or each creates a new one.',
      ),
      [
        ['single', L('Jedna wspólna sesja', 'One shared session')],
        ['new_each', L('Nowa sesja za każdym razem', 'New session each time')],
      ],
      'single',
    ),
  ]),
  page(
    'providers',
    L('Modele i dostawcy', 'Models and providers'),
    1,
    [
      select(
        'models.default_profile',
        L('Domyślny profil modelu', 'Default model profile'),
        L(
          'Lokalny działa bez kluczy; Hybryda łączy lokalny model z chmurą.',
          'Local works without keys; Hybrid combines the local model with the cloud.',
        ),
        [
          ['local', L('Lokalny', 'Local')],
          ['hybrid', L('Hybryda', 'Hybrid')],
          ['cloud', L('Chmura', 'Cloud')],
        ],
        'hybrid',
        'session',
      ),
      select(
        'models.effort',
        L('Wysiłek rozumowania', 'Reasoning effort'),
        L(
          'Wyższy daje staranniejsze odpowiedzi, ale trwa dłużej i kosztuje więcej.',
          'Higher gives more careful answers but takes longer and costs more.',
        ),
        [
          ['low', L('Niski', 'Low')],
          ['medium', L('Średni', 'Medium')],
          ['high', L('Wysoki', 'High')],
        ],
        'medium',
        'agent',
      ),
    ],
    { custom: 'providers' },
  ),
  page(
    'costs',
    L('Koszty i limity', 'Costs and limits'),
    1,
    [
      number(
        'costs.warn_at',
        L('Ostrzegaj przy zużyciu limitu', 'Warn at limit usage'),
        L(
          'Procent limitu miesięcznego, przy którym pokażemy ostrzeżenie.',
          'Share of the monthly limit at which a warning is shown.',
        ),
        80,
        50,
        100,
        5,
        '%',
      ),
      toggle(
        'costs.background_local_only',
        L('Zadania w tle tylko lokalnie', 'Background tasks local only'),
        L(
          'Osobny budżet tła: domyślnie tylko modele lokalne, bez kosztów.',
          'Separate background budget: local models only by default, no cost.',
        ),
        true,
      ),
      toggle(
        'costs.estimate_before_long',
        L('Szacuj koszt przed długim zadaniem', 'Estimate cost before long tasks'),
        L(
          'Przed dużym zadaniem pokażemy szacunek i poprosimy o zgodę.',
          'Before a large task, an estimate is shown and you are asked to confirm.',
        ),
        true,
      ),
    ],
    { custom: 'costs' },
  ),
  later('router', L('Router i reguły', 'Router and rules'), 2, [
    L('Klasy zadań i fallbacki', 'Task classes and fallbacks'),
    L('Tagi prywatności i jurysdykcji', 'Privacy and jurisdiction tags'),
  ]),
  page(
    'voice',
    L('Głos', 'Voice'),
    1,
    [
      select(
        'voice.mode',
        L('Włączanie mikrofonu', 'Microphone activation'),
        L(
          'Przełącznik (Ctrl+Shift+M) albo mówienie z przytrzymaniem Spacji (PTT).',
          'Toggle (Ctrl+Shift+M) or push-to-talk with the Space key.',
        ),
        [
          ['toggle', L('Przełącznik', 'Toggle')],
          ['ptt', L('Przytrzymaj, aby mówić (PTT)', 'Push to talk')],
        ],
        'toggle',
      ),
      toggle(
        'voice.show_pill',
        L('Pigułka głosowa', 'Voice pill'),
        L(
          'Małe okno z mówiącą agentką i stanem mikrofonu, gdy okno główne jest ukryte.',
          'A small window with the speaking agent and microphone state when the main window is hidden.',
        ),
        true,
      ),
    ],
    { custom: 'voice' },
  ),
  page('agents', L('Agentki', 'Agents'), 1, [
    number(
      'agents.max_steps',
      L('Limit kroków zadania', 'Task step limit'),
      L(
        'Agentka zatrzyma się po tylu krokach (tura modelu albo narzędzie) i pokaże raport.',
        'The agent stops after this many steps (model turn or tool) and shows a report.',
      ),
      40,
      5,
      200,
      1,
      null,
    ),
    number(
      'agents.max_minutes',
      L('Limit czasu zadania', 'Task time limit'),
      L('Najdłuższy czas jednego zadania agentki.', 'The longest time of a single agent task.'),
      15,
      1,
      120,
      1,
      'min',
    ),
    number(
      'agents.max_cost_pln',
      L('Limit kosztu zadania', 'Task cost limit'),
      L(
        'Najwyższy koszt modeli w jednym zadaniu (0 = bez limitu; limit miesięczny obowiązuje zawsze).',
        'Highest model cost of a single task (0 = no limit; the monthly limit always applies).',
      ),
      0,
      0,
      500,
      1,
      'zł',
    ),
    number(
      'agents.approval_timeout_s',
      L('Czekanie na zatwierdzenie', 'Waiting for approval'),
      L(
        'Po tym czasie bez decyzji w oknie Brokera agentka dostaje odmowę.',
        'Without a decision in the Broker window after this time, the agent is denied.',
      ),
      300,
      30,
      1800,
      30,
      's',
    ),
    toggle(
      'agents.verify_before_done',
      L('„Gotowe" po samoweryfikacji', '"Done" after self-check'),
      L(
        'Agentka sprawdza wynik narzędziami tylko do odczytu, zanim ogłosi koniec zadania.',
        'The agent checks the result with read-only tools before declaring the task done.',
      ),
      true,
    ),
  ]),
  page('permissions', L('Uprawnienia i bezpieczeństwo', 'Permissions and security'), 1, [], {
    custom: 'permissions',
  }),
  later('computer', L('Komputer', 'Computer'), 6, [
    L('Trasy: API › UIA › wizja › wejście', 'Routes: API › UIA › vision › input'),
    L('Aplikacje i przeglądarka', 'Apps and browser'),
  ]),
  later('memory', L('Pamięć', 'Memory'), 7, [
    L('Zakresy i konsolidacja nocna', 'Scopes and nightly consolidation'),
    L('Inspektor pamięci', 'Memory inspector'),
  ]),
];
