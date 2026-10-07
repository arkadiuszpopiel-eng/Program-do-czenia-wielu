# Przenoszenie danych, kopie i Zdrowie systemu

## Plik `.alfa` — przenoszenie między Twoimi komputerami

Alfa nie synchronizuje niczego sama. Żeby przenieść dane np. z komputera stacjonarnego na laptop, robisz eksport do
jednego pliku `.alfa` i import na drugim komputerze.

### Eksport

**Ustawienia → Import i eksport → Eksport** (albo paleta `Ctrl+K` → „Import i eksport .alfa”):

1. Zaznacz zakres: konfiguracja wspólna, agentki i biblie głosów, obsady ról, wybrane zakresy pamięci, wybrane sesje,
   a opcjonalnie także **artefakty** wybranych sesji (pliki z panelu Pliki), **logi** (sekrety redagowane) i **nakładkę
   tej maszyny** (urządzenia, profil głosu, limity). Artefakty przywrócone z paczki trafiają do
   `…\Alfa\Sesje\Import`.
2. Opcjonalnie **Zaszyfruj paczkę hasłem** (hasło dwa razy, co najmniej 8 znaków).
3. **Eksportuj…** — wybierz miejsce zapisu.

Pojedynczą sesję wyeksportujesz z jej menu („Eksportuj do .alfa”) albo komendą `/eksport`. Umiejętności mają własny
eksport na stronie **Umiejętności**.

**Klucze API nigdy nie trafiają do zwykłej paczki** — Alfa sprawdza każdy plik przed zapisem i przerywa eksport, gdy
znajdzie sekret. Klucze zostają w Menedżerze poświadczeń Windows tej maszyny — na nowym komputerze dodaj je ponownie
w **Ustawienia → Konta** (paczka sekretów ze starszej wersji Alfy nie jest importowana). Sesje prywatne nie są
eksportowane.

### Import

1. **Wybierz plik .alfa…** (przy paczce zaszyfrowanej podaj hasło i kliknij „Odszyfruj”).
2. Podgląd bez zmian: skąd paczka i z kiedy, a przy każdym elemencie stan — nowe, bez zmian, zmienione, kolizja.
3. Tryb: **Dodaj** (tylko nowe), **Scal** (nowe + aktualizacja zmienionych) albo **Zastąp** (paczka nadpisuje lokalne
   — Alfa poprosi o potwierdzenie). Przy kolizji: „Zostaw lokalne”, „Weź z paczki” albo „Zachowaj oba”.
4. **Importuj.**

Przed każdym importem Alfa robi automatyczną migawkę. Przycisk **Cofnij import** przywraca stan sprzed importu.

## Kopie zapasowe

**Ustawienia → Import i eksport → Kopie zapasowe** — zaplanowany eksport `.alfa` (ten sam format co eksport ręczny):

1. **Wybierz katalog…** — np. inny dysk albo folder w OneDrive (nie katalog z danymi Alfy).
2. Włącz **Twórz kopie automatycznie**, wybierz odstęp (co 6 h … co tydzień) i liczbę zachowanych kopii — najstarsze
   są usuwane. Opcjonalnie dołącz artefakty i logi; domyślnie kopie nie powstają na baterii ani przy pełnym ekranie.
3. Opcjonalnie **hasło kopii** (co najmniej 8 znaków; trzymane w Menedżerze poświadczeń Windows) — wtedy kopie są
   zaszyfrowane i obejmują także sesje prywatne. Bez hasła sesje prywatne są pomijane.

Kopia obejmuje konfigurację wspólną i nakładkę tej maszyny, agentki, obsady, reguły, umiejętności, wszystkie sesje
i zakresy pamięci. **Klucze API i sekrety nigdy nie trafiają do kopii.** **Utwórz kopię teraz** robi kopię od razu;
nieudana kopia z harmonogramu pokazuje powiadomienie i ponawia się po godzinie.

Przy każdej kopii na liście: **Sprawdź** — test przywracania bez zmian (otwarcie, sumy kontrolne, odszyfrowanie,
podgląd), **Przywróć…** — podgląd importu tej kopii (dalej jak przy imporcie: tryb, kolizje, migawka i „Cofnij
import”).

## Zdrowie systemu

**Ustawienia → Zdrowie systemu.** Diagnosta obserwuje moduły i błędy (np. odrzucony klucz, limit zapytań, awarie
modułów, brak miejsca) i:

- pokazuje stan ogólny: „Wszystko działa”, „Działa z problemami”, „Awaria modułu” albo „Tryb bezpieczny”;
- **Sprawdź teraz** — natychmiastowe sprawdzenie;
- **Propozycje napraw** — każda z ryzykiem, opisem zmian i planem cofnięcia; przyciski **Napraw** i **Odrzuć**.
  Naprawy dotyczące Jądra zatwierdza się wyłącznie w oknie Brokera;
- **Incydenty**, **Wykonane naprawy** (każdą można **Cofnij**), sprawy **wymagające Twojej decyzji** i tabela modułów;
- ustawienie **Samodzielność Diagnosty** — co naprawia sam (zawsze z „Cofnij”), a co tylko proponuje (zmiana działa
  od następnego uruchomienia).

### Ulepszacz

Na tej samej stronie. Na podstawie dziennika proponuje drobne zmiany ustawień (modelem lokalnym, gdy komputer jest
bezczynny, albo po „Przeanalizuj teraz”). Każda zmiana pokazuje: ustawienie, było → będzie, czy zawęża, czy poszerza.
Przechodzi ukryty test jakości i czeka na Twoje **Zatwierdź ten diff**; wdrożoną zmianę wycofasz przyciskiem
**Wycofaj**. Ulepszacz nie może zmienić zasad bezpieczeństwa, uprawnień, prywatności, budżetów ani progów testów.

### Evale

Lista zamrożonych zestawów testowych z kontrolą integralności („zgodna z hashem” / „naruszona”, przycisk
**Sprawdź**) i werdykty bramki jakości.

## Gdzie są logi

`Win+R` → wpisz `%LOCALAPPDATA%\Alfa\logs` → `Enter`.

- **Dziennik diagnostyczny** — zwykły tekst (otworzysz Notatnikiem): błędy, ostrzeżenia, start modułów,
  STOP WSZYSTKIEGO. Pliki `alfa.<data>.000.log` (aplikacja), `alfa-broker.…`, `alfa-watchdog.…`,
  `alfa-broker-ui.…`. Jedna linia = jedno zdarzenie; czas w UTC (w Polsce +1 h zimą, +2 h latem). Nowy plik
  codziennie i po 10 MB, najwyżej 14 plików na proces; pliki starsze niż 7 dni Alfa usuwa sama.
- **Zdarzenia** (wywołania modeli, narzędzia, głos, diagnostyka) — podkatalogi `model_calls`, `tools_gui`, `voice`,
  `diagnostics` z plikami `.ndjson`. Tylko te trafiają do eksportu `.alfa`, gdy zaznaczysz **Logi**.
- **Usługa Brokera** z instalatora pisze swój dziennik w `%LOCALAPPDATA%` konta usługi, nie w Twoim profilu.

Więcej szczegółów (np. do zgłoszenia błędu): w pliku `%APPDATA%\Alfa\config\shared.toml` dopisz

```toml
[logs]
level = "debug"   # error, warn, info (domyślnie), debug, trace
file_days = 7     # ile dni trzymać dziennik (1–90)
```

i uruchom Alfę ponownie (albo jednorazowo: zmienna środowiskowa `ALFA_LOG=debug` — ma pierwszeństwo). Wróć do
`info`, gdy skończysz — `debug` szybciej zapełnia pliki.

W dzienniku **nie ma** kluczy API, haseł, treści rozmów, transkrypcji ani zrzutów ekranu: pola z sekretami są
zamazywane (`[REDACTED]`), a treść pomijana (`[pominięto: N znaków]`). Mogą być nazwy plików i ścieżki — przejrzyj
plik, zanim go komuś wyślesz.
