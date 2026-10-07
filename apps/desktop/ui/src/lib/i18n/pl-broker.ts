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
  'broker.banner.details': 'Szczegóły',
  'broker.approval.lost':
    'Broker jest niedostępny — tej prośby nie da się teraz zatwierdzić; agentka dostanie odmowę.',
} as const satisfies Record<string, Message>;
