// Słownik PL: Broker (Ustawienia → Uprawnienia i bezpieczeństwo, baner bezpiecznego stanu,
// karta „czeka na zatwierdzenie”). Zatwierdzanie wyłącznie w oknie Brokera — nigdy tutaj.
import type { Message } from './core';

export const plBroker = {
  'broker.title': 'Broker (Jądro bezpieczeństwa)',
  'broker.region': 'Stan Brokera',
  'broker.mode.service': 'Usługa Brokera na osobnym koncie Windows',
  'broker.mode.portable': 'Tryb przenośny — Broker bez osobnego konta',
  'broker.mode.in_process': 'Broker w procesie aplikacji (tryb deweloperski)',
  'broker.mode.unavailable': 'Brak Brokera',
  'broker.state.connected': 'Połączono',
  'broker.state.connecting': 'Łączenie…',
  'broker.state.lost': 'Połączenie zerwane — bezpieczny stan',
  'broker.window.on': 'Okno zatwierdzeń: działa — tam zatwierdzasz każdą prośbę.',
  'broker.window.off': 'Okno zatwierdzeń: niedostępne — prośby o zgodę są odrzucane.',
  'broker.watchdog.on': 'STOP WSZYSTKIEGO (Ctrl+Shift+F12): obsługuje watchdog, poza aplikacją.',
  'broker.watchdog.off':
    'STOP WSZYSTKIEGO (Ctrl+Shift+F12): awaryjnie w aplikacji — watchdog nie działa.',
  'broker.isolation.full': 'Izolacja: pełna (osobne konto, okno chronione przez UIPI).',
  'broker.isolation.weak': 'Izolacja: słabsza — Broker i jego okno działają na Twoim koncie.',
  'broker.service.hint':
    'Pełną izolację daje usługa Brokera na osobnym koncie Windows. Instaluje ją raz administrator (jednorazowe potwierdzenie UAC) — instrukcja w przewodniku „Bezpieczeństwo”.',
  'broker.banner.lost':
    'Połączenie z Brokerem zerwane. Bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane; Alfa ponawia połączenie.',
  'broker.banner.unavailable':
    'Brak izolowanego Brokera — agentki nie wykonają niczego, co wymaga zgody. Zainstaluj Alfę ponownie.',
  'broker.banner.connecting':
    'Łączenie z Brokerem… do czasu połączenia prośby o zgodę są odrzucane.',
  'broker.banner.watchdog':
    'Watchdog nie działa — STOP WSZYSTKIEGO (Ctrl+Shift+F12) obsługuje awaryjnie aplikacja.',
  'broker.banner.dev': 'Tryb deweloperski: Broker w procesie aplikacji, bez okna zatwierdzeń.',
  'broker.banner.unknown':
    'Nie udało się odczytać stanu Brokera — nie wiadomo, czy okno zatwierdzeń działa. Sprawdź Szczegóły albo uruchom Alfę ponownie.',
  'broker.banner.details': 'Szczegóły',
  'broker.approval.lost':
    'Broker jest niedostępny — tej prośby nie da się teraz zatwierdzić; agentka dostanie odmowę.',
  // Tryb pracy z paska tytułu (`ModeMenu`, decyzja 27.10).
  'titlebar.mode': 'Tryb pracy: {mode}. Zmień tryb',
  'titlebar.modeTip': 'Tryb pracy agentek — kliknij, aby zmienić (1–4)',
  'mode.title': 'Tryb pracy',
  'status.changeMode': 'Zmień tryb pracy',
  'mode.L0.name': 'Tylko podgląd',
  'mode.L0.desc': 'Agentka tylko czyta i podpowiada, nic nie zmienia.',
  'mode.L1.name': 'Pytaj o wszystko',
  'mode.L1.desc': 'Każda zmiana wymaga Twojego „tak”.',
  'mode.L2.name': 'Pytaj o ryzykowne',
  'mode.L2.desc': 'Pyta przy usuwaniu, wysyłaniu danych i instalacji.',
  'mode.L3.name': 'Automatycznie',
  'mode.L3.desc': 'Działa sama; pyta tylko o rzeczy nieodwracalne poza zakresem.',
  'mode.L4.name': 'Pełna automatyka',
  'mode.L4.desc': 'Bez pytań. Wybierasz raz — zostaje po restarcie.',
  'mode.note':
    'Dotyczy wszystkich rozmów. Podniesienie potwierdzasz raz w oknie Brokera. Zawsze zostają: Ctrl+Shift+F12, dziennik i pytanie przed wysyłką po obcej treści i przy poleceniu obcym albo niepewnie rozpoznanym głosem.',
  'mode.more': 'Więcej w Ustawieniach → Uprawnienia',
  'mode.applied': 'Tryb pracy: {mode}.',
  'mode.requested': 'Potwierdź „{mode}” w oknie Brokera — wystarczy raz.',
  'mode.raised': 'Tryb pracy: {mode} — zatwierdzony i zapamiętany.',
  'mode.notApplied': 'Broker nie przyjął trybu „{mode}” — obowiązuje „{current}”.',
  'mode.sessionOwn':
    'Ta rozmowa ma własny tryb: {mode}. Wybór tutaj zmienia tryb pozostałych rozmów; tryb tej rozmowy zmienisz w Uprawnieniach.',
  'mode.badge': 'Włączona Pełna automatyka — agentki działają bez pytań. Kliknij, aby zmienić tryb',
} as const satisfies Record<string, Message>;
