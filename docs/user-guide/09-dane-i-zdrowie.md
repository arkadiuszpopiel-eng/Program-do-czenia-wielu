# Przenoszenie danych, kopie i Zdrowie systemu

## Plik `.alfa` — przenoszenie między Twoimi komputerami

Alfa nie synchronizuje niczego sama. Żeby przenieść dane np. z komputera stacjonarnego na laptop, robisz eksport do
jednego pliku `.alfa` i import na drugim komputerze.

### Eksport

**Ustawienia → Import i eksport → Eksport** (albo paleta `Ctrl+K` → „Import i eksport .alfa”):

1. Zaznacz zakres: konfiguracja wspólna, agentki i biblie głosów, obsady ról, wybrane zakresy pamięci, wybrane sesje.
   Artefakty, logi i ustawienia tej konkretnej maszyny są **jeszcze niedostępne** w eksporcie.
2. Opcjonalnie **Zaszyfruj paczkę hasłem** (hasło dwa razy, co najmniej 8 znaków).
3. **Eksportuj…** — wybierz miejsce zapisu.

Pojedynczą sesję wyeksportujesz z jej menu („Eksportuj do .alfa”) albo komendą `/eksport`. Umiejętności mają własny
eksport na stronie **Umiejętności**.

**Klucze API nigdy nie trafiają do zwykłej paczki** — Alfa sprawdza każdy plik przed zapisem i przerywa eksport, gdy
znajdzie sekret. Jeśli chcesz przenieść klucze, użyj osobnej opcji **Eksport sekretów**: tworzy oddzielną paczkę,
zawsze zaszyfrowaną hasłem. Sesje prywatne nie są eksportowane.

### Import

1. **Wybierz plik .alfa…** (przy paczce zaszyfrowanej podaj hasło i kliknij „Odszyfruj”).
2. Podgląd bez zmian: skąd paczka i z kiedy, a przy każdym elemencie stan — nowe, bez zmian, zmienione, kolizja.
3. Tryb: **Dodaj** (tylko nowe), **Scal** (nowe + aktualizacja zmienionych) albo **Zastąp** (paczka nadpisuje lokalne
   — Alfa poprosi o potwierdzenie). Przy kolizji: „Zostaw lokalne”, „Weź z paczki” albo „Zachowaj oba”.
4. **Importuj.**

Przed każdym importem Alfa robi automatyczną migawkę. Przycisk **Cofnij import** przywraca stan sprzed importu.

## Kopie zapasowe

Automatyczne kopie według harmonogramu są **jeszcze niedostępne** w aplikacji. Do tego czasu rób kopię ręcznie:
eksport pełnego zakresu, najlepiej zaszyfrowany hasłem, na inny dysk. Przywrócenie = import tej paczki.

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
