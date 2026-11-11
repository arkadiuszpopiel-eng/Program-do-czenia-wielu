# Kreator: wspólny profil Jarvisa, #44

Przy bibliotece własnych agentek dostępny jest ten sam wspólny wybór Standardowy/Jarvis co
w ustawieniach Agentek. Własne persony dziedziczą go; opis wskazuje też wbudowane z wyborem
„Jak ustawienie wspólne” i pierwszeństwo ich osobnych ustawień. Zmiana obowiązuje od następnej
odpowiedzi. Mikrofon właściciel włącza osobno w Głosie. Teksty są po polsku i angielsku.

To istniejący klucz `agents.interaction_mode`; kontrakty, uprawnienia i przypisanie ról są
dotychczasowe. Pełne UI osobnych profili i zarządzania obsadą własnych person pozostaje odrębnym
zakresem. [Własna Luna na rzeczywistym modelu](2026-11-09-live-custom.md) potwierdza dziedziczenie
wewnątrz Core, osobno od tych testów interfejsu.

## Sprawdzenie przed pełnym E2E

- Najpierw test na dotychczasowym buildzie:1 nieudany,8 niewykonanych po `--max-failures=1`;
  biblioteka otwarta, brak kontrolki „Wspólny profil współpracy” w5000ms. Komunikat zatrzymania
  runnera zachowano. Stary bundle bd389207 miał identyczne źródła produkcyjne UI i packages z974ae45e.
- Po zmianie osobny build w tym worktree:18,25s. **9/9 E2E**,26,4s: cztery style × dwa motywy,
  zapis globalnego Jarvisa, odtworzenie wyboru po nawigacji, dziedzicząca Beta i osobny Standard
  Alfy, powrót do wspólnego Standardu, axe oraz przełączenie języka na angielski.
- Svelte/TypeScript0 błędów i0 ostrzeżeń, ESLint i Prettier zielone.
- Budżety: JS134,8/150KB gzip, CSS13,9/30KB; Szybkie pytanie39,2/40KB, pigułka8,0/8KB.
  Kontrolka trafia do leniwego widoku Ustawień.
- Jednostkowe UI osobno, bez modelu/Rust:573/573,103 pliki,28,99s.
- Rozmiary po dodaniu nowych plików do indeksu:3489 plików,264 crate'y,0 naruszeń.

Logi `/workspace/scratch/builder-jarvis-{red,tests,check,build,unit}.log`.
Pełny nowy zestaw **899/899** E2E zaliczony na zamrożonym `af5a2bc5`,0 pominiętych,0 niestabilnych,
kod0,38,5minuty. [Podsumowanie, komenda i SHA256](artifacts/ui-builder-full-2026-11-09.json).
Surowy raport985719 bajtów zachowano w `/workspace/scratch/ui-builder-full/report.json`.
Wcześniejszy pełny CI159890/890 dotyczy169a6004 (UI produkcyjne równoważne974ae45e), bez tej kontrolki; nie zastępuje nowej próby.
To Linux/Chromium z atrapą backendu, bez odbioru Windows/WebView2 i mikrofonu.
