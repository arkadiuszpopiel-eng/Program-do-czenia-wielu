# Audyt interfejsu — obsługa błędów, stany ładowania, dostępność, wąskie okno (2026-10-07)

**Powód.** Test na laptopie (2026-10-07): w kreatorze kluczy „klucz znika i nic się nie dzieje” — odrzucona
komenda rdzenia bez komunikatu. Pytanie właściciela: czy inne funkcje mają podobne problemy, i prośba
o lepszy interfejs. Zamiast przeglądu „na oko” — cztery audyty automatyczne, które przechodzą **każdą
stronę ustawień (29) i każdy panel (7)**, są częścią testów E2E (Playwright, CI „UI (Svelte 5)”) i mają
potwierdzoną czułość (test mutacyjny: celowo zepsuty widok musi zostać wskazany).

## Metoda

| Audyt                         | Plik                       | Jak                                                                                                                                                   | Kryterium                                                                                                           |
| ----------------------------- | -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Błędy rdzenia                 | `e2e/core-errors.spec.ts`  | scenariusz atrapy `core-errors`: każda komenda poza startem (`app.bootstrap`, `sessions.list`, `settings.schema`) odrzuca `CommandError` jak rdzeń   | 0 wyjątków strony, 0 odrzuceń, które dotarły dopiero do siatki bezpieczeństwa (`main.ts` → `[alfa] nieobsłużone odrzucenie`) |
| Wolny rdzeń                   | `e2e/slow-core.spec.ts`    | scenariusz `slow-core`: odpowiedź po 1,5 s, rejestr wywołań `window.__alfaCalls` (`fake/fault.ts`)                                                   | widok, który po otwarciu czeka na rdzeń, pokazuje stan ładowania (`role=status` / `aria-busy`) po ≤ 700 ms           |
| Dostępność                    | `e2e/a11y-all.spec.ts`     | axe (WCAG 2.0/2.1/2.2 A i AA) na każdej stronie i panelu, motyw jasny i ciemny                                                                        | 0 naruszeń critical/serious                                                                                          |
| Wąskie okno                   | `e2e/responsive.spec.ts`   | każda strona ustawień przy 720 i 480 px; pasek tytułu przy 1280/720/480/400 px (minimum okna)                                                        | nic poza oknem; w pasku tytułu nic poza paskiem i żaden tekst złamany na kilka linii                                |
| Skan kodu                     | `grep` (raport niżej)      | `catch {}`, `.catch(() => undefined)`, `void client.*` bez obsługi                                                                                    | każde wystąpienie uzasadnione albo poprawione                                                                        |

Test mutacyjny (2026-10-07): usunięcie obsługi błędu w `BundlesSection` → audyt błędów wskazał „Modele i silniki:
engines.bundles”; przywrócenie panelu Zadania sprzed poprawki → audyt ładowania wskazał „Alt+7: tasks.list”; stara
kapsuła „steruje ekranem” → audyt paska tytułu wskazał „tekst w 3 liniach” (480 px) i wyjście poza okno (400 px).

## Wyniki — strony ustawień

Kolumna „Błąd rdzenia”: co widać przy odrzuconej komendzie (liczba komunikatów „Nie udało się wczytać… Ponów”).
Strony z „—” nie wołają rdzenia przy otwarciu (ustawienia z `settings.schema` i wartości z `app.bootstrap`).

| Strona                          | Błąd rdzenia | Stan ładowania (przed → po) | axe (jasny/ciemny) | 720 / 480 px |
| ------------------------------- | ------------ | --------------------------- | ------------------ | ------------ |
| Ogólne                          | —            | — (nie czeka)               | 0 / 0              | OK / OK      |
| Modele i dostawcy               | 2            | brak → szkielet             | 0 / 0              | OK / OK      |
| Modele i silniki                | 2            | „Ładowanie pakietów…” (role=status) | 0 / 0      | OK / OK      |
| Koszty i limity                 | 1            | brak → szkielet / `aria-busy` przy odświeżaniu | 0 / 0 | OK / OK  |
| Router i reguły (fala 2)        | —            | —                           | 0 / 0              | OK / OK      |
| Głos                            | 3            | „Ładowanie” (role=status)   | 0 / 0              | OK / OK      |
| Agentki                         | —            | —                           | 0 / 0              | OK / OK      |
| Umiejętności                    | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Wtyczki                         | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Kreator agentek                 | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Zadania w tle i wyzwalacze      | 2            | brak → szkielet             | 0 / 0              | OK / OK      |
| Reguły Marszałka                | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Uprawnienia i bezpieczeństwo    | 1            | OK                          | 0 / 0              | OK / OK      |
| Komputer                        | 1            | OK                          | 0 / 0              | OK / OK      |
| Pamięć                          | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Sesje i okna                    | —            | —                           | 0 / 0              | OK / OK      |
| Pliki                           | —            | —                           | 0 / 0              | OK / OK      |
| Logi i prywatność (fala 3)      | —            | —                           | 0 / 0              | OK / OK      |
| Zdrowie systemu                 | 3            | brak → szkielet             | 0 / 0              | OK / OK      |
| Moduły (fala 2)                 | —            | —                           | 0 / 0              | OK / OK      |
| Urządzenia                      | 1            | brak → szkielet             | 0 / 0              | OK / OK      |
| Import i eksport                | 2            | brak → szkielet (zakresy pamięci, kopie) | 0 / 0 | OK / OK      |
| Wygląd, Skróty, Powiadomienia, Język, Zaawansowane | — | —                | 0 / 0              | OK / OK      |
| Aktualizacje                    | 1            | OK                          | 0 / 0              | OK / OK      |
| O programie                     | 1            | OK                          | 0 / 0              | OK / OK      |

## Wyniki — panele i rozmowa

| Panel (skrót)            | Błąd rdzenia | Stan ładowania (przed → po)                                   | axe   |
| ------------------------ | ------------ | ------------------------------------------------------------- | ----- |
| Agentki (Alt+1)          | OK           | katalog roboczy: brak → szkielet                              | 0 / 0 |
| Oś czasu (Alt+2)         | OK           | **„Brak zdarzeń” w trakcie ładowania (błąd)** → szkielet      | 0 / 0 |
| Pliki (Alt+3)            | OK           | brak → szkielet                                               | 0 / 0 |
| Pamięć (Alt+4)           | OK           | brak → szkielet                                               | 0 / 0 |
| Ekran (Alt+5)            | OK           | „Ładowanie” bez roli → szkielet z `role=status`               | 0 / 0 |
| Głos (Alt+6)             | OK           | „Ładowanie” bez roli → szkielet z `role=status`               | 0 / 0 |
| Zadania (Alt+7)          | OK           | brak → szkielet                                               | 0 / 0 |
| Rozmowa: wysłanie        | OK           | szkic zostaje w polu (sprawdzane w teście)                    | —     |

## Co poprawiono

1. **Obsługa błędów** (commity `94e86bb`, `ffc4e2f`, `3c861f5`): jeden wzorzec w całym UI — `attempt`/`showError`
   (toast z treścią z rdzenia), `load`/`Loadable` + `LoadFailed` („Nie udało się wczytać… Ponów”) zamiast pustych
   list i „Ładowanie…” na zawsze; centralna zamiana błędu IPC na `Error` (`toError`); siatka bezpieczeństwa dla
   odrzuceń bez obsługi. Audyt potwierdza: 0 widoków zależnych od siatki.
2. **Stany ładowania**: wspólny komponent `components/shell/Loading.svelte` (szkielet dopiero po 300 ms — szybka
   odpowiedź nie miga; `role=status` dla czytników ekranu) w 11 stronach i 7 panelach, które przy wolnym rdzeniu
   pokazywały pustą kartę.
3. **Oś czasu**: w trakcie ładowania pokazywała „brak zdarzeń” (mylący pusty stan) — teraz szkielet do pierwszej
   odpowiedzi.
4. **Pasek tytułu w wąskim oknie**: kapsuła „Delta steruje ekranem” łamała się na 3 linie (480 px) i wypychała
   przyciski poza okno (400 px) — teraz zwęża się z wielokropkiem, a poniżej 720 px zostaje ikona (pełna etykieta
   w `aria-label`).
5. **Esc przy czytaniu na głos**: błąd zatrzymania był połykany — teraz toast.
6. **Modele i silniki** (commity `3c861f5`, `3d8afe4`): pakiety 1–6 dobrane do sprzętu, „Napraw” elementu
   z potwierdzeniem, uwagi o jakości według norm, ustawienia silników z opisami i zaleceniami, nagłówek „Ustawienia
   szczegółowe” na stronach z własnym widokiem.

## Skan kodu — świadome wyjątki

Każde `.catch(() => undefined)` / `null` po skanie (2026-10-07) — zostaje, bo błąd ma inną, widoczną drogę albo
jest sprzątaniem po zamknięciu widoku:

| Miejsce                                                                 | Dlaczego bez toastu                                                                                   |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `pill/main.ts` — `app_bootstrap` pigułki                                 | pigułka startuje z polskim i motywem systemu; brak rdzenia pokazuje jej własny stan                   |
| `WhatsNewDialog` — `dismissWhatsNew`                                     | zamknięcie okna „Co nowego” nie może się nie udać z perspektywy użytkownika; pokaże się ponownie      |
| `TerminalDialog` — `input`, `resize`, `close` (×4)                       | strumień znaków i zmiany rozmiaru; awaria sesji przychodzi jako zdarzenie wyjścia terminala            |
| `VoicePage`, `MicStep` — `stopMicTest` przy zamykaniu widoku (×2)        | sprzątanie po odmontowaniu; widok, który mógłby pokazać błąd, już nie istnieje                        |

Poprawione w tym audycie: `voice.stopReading` (Esc) — był na tej liście, teraz pokazuje błąd.

## Jak uruchomić

```bash
cd apps/desktop/ui && pnpm build
npx playwright test e2e/core-errors.spec.ts e2e/slow-core.spec.ts e2e/a11y-all.spec.ts e2e/responsive.spec.ts
# podgląd ręczny: pnpm dev → http://localhost:5173/?scenario=core-errors (albo slow-core)
```
