# Poprawki agentek i interfejsu — 24.10.2026

Zakres: limit 5–500 kroków, konfiguracja narzędzi każdej persony, maksymalny
dostęp Alfy, czytelny edytor Pipeline, osobne menu agentek na środku belki oraz
nawigacja ustawień po prawej. Istniejące zmiany w checkout pozostają zachowane.

Kryteria:
1. UI i rdzeń przyjmują 500 kroków; wartości ponad limit nie zwiększają budżetu.
2. Każda agentka ma profile: według ról, tylko odczyt, pełny, własny, wyłączony.
   Własny profil wybiera grupy narzędzi i tryb odczytu. Alfa domyślnie pełny.
   Ustawienia obowiązują w zwykłym wykonaniu i delegacji. Broker nadal rozstrzyga akcje.
3. Duże repozytorium nie blokuje edycji pojedynczego pliku przez fs; błąd kopii
   powłoki wskazuje ten sposób działania i nie sugeruje braku uprawnień.
4. Pipeline ma niekurczące się etapy, responsywny układ, stan błędu/ponowienie,
   zachowanie szkicu po przypadkowym przełączeniu i brak nakładających się akcji.
5. Każda ikona agentki otwiera menu z jej nazwą, rolami, skrótami do konfiguracji,
   uprawnień i rozmowy. Obsługa myszą i klawiaturą.
6. Nawigacja ustawień jest po prawej, bez utraty obsługi wąskiego okna.

Plan weryfikacji: testy regresji budżetu i filtrów narzędzi, Vitest, svelte-check,
build, testy Playwright z rzeczywistym renderowaniem UI i atrapą backendu,
testy Rust dotkniętych modułów oraz osobny przegląd świeżym kontekstem.
Testy z atrapą nie stanowią potwierdzenia wykonania płatnego modelu ani zgód Brokera.
