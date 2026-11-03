// Słownik PL: Broker (Ustawienia → Uprawnienia i bezpieczeństwo, baner bezpiecznego stanu,
// karta „czeka na zatwierdzenie”). Zatwierdzanie wyłącznie w oknie Brokera — nigdy tutaj.
import type { Message } from './core';

export const plBroker = {
  'broker.title': 'Sędzia Cieni (Jądro bezpieczeństwa)',
  'broker.region': 'Stan Sędziego Cieni',
  'broker.mode.service': 'Usługa Sędziego Cieni na osobnym koncie Windows',
  'broker.mode.portable': 'Tryb przenośny — Sędzia Cieni bez osobnego konta',
  'broker.mode.in_process': 'Sędzia Cieni w procesie aplikacji (tryb deweloperski)',
  'broker.mode.unavailable': 'Brak Sędziego Cieni',
  'broker.state.connected': 'Połączono',
  'broker.state.connecting': 'Łączenie…',
  'broker.state.lost': 'Połączenie zerwane — bezpieczny stan',
  'broker.window.on': 'Okno zatwierdzeń: działa — tam zatwierdzasz każdą prośbę.',
  'broker.window.off': 'Okno zatwierdzeń: niedostępne — prośby o zgodę są odrzucane.',
  'broker.watchdog.on': 'STOP WSZYSTKIEGO (Ctrl+Shift+F12): obsługuje watchdog, poza aplikacją.',
  'broker.watchdog.off':
    'STOP WSZYSTKIEGO (Ctrl+Shift+F12): awaryjnie w aplikacji — watchdog nie działa.',
  'broker.isolation.full': 'Izolacja: pełna (osobne konto, okno chronione przez UIPI).',
  'broker.isolation.weak': 'Izolacja: słabsza — Sędzia Cieni i jego okno działają na Twoim koncie.',
  'broker.service.hint':
    'Pełną izolację daje usługa Sędziego Cieni na osobnym koncie Windows. Instaluje ją raz administrator (jednorazowe potwierdzenie UAC) — instrukcja w przewodniku „Bezpieczeństwo”.',
  'broker.banner.lost':
    'Połączenie z Sędzią Cieni zerwane. Bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane; Alfa ponawia połączenie.',
  'broker.banner.unavailable':
    'Brak izolowanego Sędziego Cieni — agentki nie wykonają niczego, co wymaga zgody. Zainstaluj Alfę ponownie.',
  'broker.banner.connecting':
    'Łączenie z Sędzią Cieni… do czasu połączenia prośby o zgodę są odrzucane.',
  'broker.banner.watchdog':
    'Watchdog nie działa — STOP WSZYSTKIEGO (Ctrl+Shift+F12) obsługuje awaryjnie aplikacja.',
  'broker.banner.dev':
    'Tryb deweloperski: Sędzia Cieni w procesie aplikacji, bez okna zatwierdzeń.',
  'broker.banner.unknown':
    'Nie udało się odczytać stanu Sędziego Cieni — nie wiadomo, czy okno zatwierdzeń działa. Sprawdź Szczegóły albo uruchom Alfę ponownie.',
  'broker.banner.details': 'Szczegóły',
  'broker.approval.lost':
    'Sędzia Cieni jest niedostępny — tej prośby nie da się teraz zatwierdzić; agentka dostanie odmowę.',
  // Tryb pracy z paska tytułu (`ModeMenu`, decyzja 27.10).
  'titlebar.mode': 'Tryb pracy: {mode}. Zmień tryb',
  'titlebar.modeTip': 'Tryb pracy agentek — kliknij, aby zmienić (1–5)',
  'mode.title': 'Tryb pracy',
  'status.changeMode': 'Zmień tryb pracy',
  'mode.L0.name': 'Tylko podgląd',
  'mode.L0.desc': 'Agentka tylko czyta i podpowiada, nic nie zmienia.',
  'mode.L1.name': 'Pytaj o wszystko',
  'mode.L1.desc': 'Każda zmiana wymaga Twojego „tak”.',
  'mode.L2.name': 'Akceptuj edycje',
  'mode.L2.desc': 'Zmiany w folderze pracy bez pytania; pliki poza nim i Internet — pyta.',
  'mode.L3.name': 'Automatycznie',
  'mode.L3.desc': 'Działa sama; pyta tylko o rzeczy nieodwracalne poza zakresem.',
  'mode.L4.name': 'Pełna automatyka',
  'mode.L4.desc': 'Bez pytań. Wybierasz raz — zostaje po restarcie.',
  'mode.note':
    'Dotyczy wszystkich rozmów. Podniesienie potwierdzasz raz w oknie Sędziego Cieni. Na Pełnej automatyce okno Sędziego Cieni nie pokazuje się wcale, aż obniżysz tryb. Zawsze zostają: Ctrl+Shift+F12, dziennik i twarde blokady Jądra.',
  'mode.more': 'Więcej w Ustawieniach → Uprawnienia',
  'mode.applied': 'Tryb pracy: {mode}.',
  'mode.requested': 'Potwierdź „{mode}” w oknie Sędziego Cieni — wystarczy raz.',
  'mode.raised': 'Tryb pracy: {mode} — zatwierdzony i zapamiętany.',
  'mode.notApplied': 'Sędzia Cieni nie przyjął trybu „{mode}” — obowiązuje „{current}”.',
  'mode.sessionOwn':
    'Ta rozmowa ma własny tryb: {mode}. Wybór tutaj zmienia tryb pozostałych rozmów; tryb tej rozmowy zmienisz w Uprawnieniach.',
  'mode.badge': 'Włączona Pełna automatyka — agentki działają bez pytań. Kliknij, aby zmienić tryb',
  'broker.grants.title': 'Zgody „zawsze w tym zakresie”',
  'broker.grants.intro':
    'Udzielasz ich w oknie Sędziego Cieni i sam wybierasz czas: 1 h, 3 h, doba albo do odwołania. Zgoda dotyczy tylko jednej sesji i jednego rodzaju działań; nie zmienia trybu pracy. Odwołanie działa od razu, a STOP (Ctrl+Shift+F12) kasuje wszystkie zgody.',
  'broker.grants.empty': 'Brak zgód — o każde takie działanie Sędzia Cieni zapyta.',
  'broker.grants.until': 'do {when}',
  'broker.grants.untilRevoked': 'do odwołania',
  'broker.grants.allAgents': 'Wszystkie agentki w sesji',
  'broker.grants.revoke': 'Odwołaj',
  'broker.grants.revokeLabel': 'Odwołaj zgodę: {scope}',
  'broker.grants.revoked': 'Zgoda odwołana — Sędzia Cieni znów zapyta.',
  'broker.grants.gone':
    'Tej zgody już nie było (wygasła albo odwołano ją wcześniej) — lista odświeżona.',
  'broker.grants.readFailed':
    'Nie udało się odczytać zgód od Sędziego Cieni — lista może być nieaktualna. Spróbuję ponownie za chwilę.',
} as const satisfies Record<string, Message>;
