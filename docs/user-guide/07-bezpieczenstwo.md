# Bezpieczeństwo: poziomy, Broker, kill-switch, cofanie

Nad wszystkim, co robią agentki, czuwa **Broker** — część Alfy, której agentki nie mogą zmienić. Każde działanie
(odczyt pliku, polecenie, kliknięcie w oknie) dostaje od niego jednorazową zgodę albo odmowę, a decyzja trafia do
dziennika Audytu, którego nie da się po cichu poprawić.

## Poziomy autonomii L0–L4

„Jak bardzo agentka działa sama”. Domyślnie **L3**.

| Poziom                      | Co to znaczy                                                                                                                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L0 Podgląd                  | Agentka tylko czyta i podpowiada, nic nie zmienia.                                                                                                                              |
| L1 Pytaj o wszystko         | Każda zmiana wymaga Twojego „tak”.                                                                                                                                              |
| L2 Pytaj o ryzykowne        | Drobiazgi robi sama; pyta przy usuwaniu, wysyłaniu danych na zewnątrz i instalacji.                                                                                             |
| L3 Bardzo wysoka (domyślny) | Działa sama w Twoim profilu i we wskazanych aplikacjach; pyta przy rzeczach nieodwracalnych poza zakresem albo gdy działa na podstawie niezaufanej treści (mail, strona, plik). |
| L4 Maks                     | Nie pyta o nic poza twardymi blokadami i potwierdzeniem destrukcji zleconej głosem.                                                                                             |

**Ustawienia → Uprawnienia i bezpieczeństwo**: wybierz poziom i zakres zmiany (globalnie albo tylko ta sesja).
**Obniżenie** działa od razu. **Podniesienie** potwierdza się wyłącznie w oknie Brokera (gdy okna nie ma — patrz
niżej — kończy się odmową). Agentka nigdy nie podniesie poziomu sama.

## Czego nie da się zrobić na żadnym poziomie (także L4)

- wyłączyć ani zmienić Audytu, watchdoga, kill-switcha i zasad bezpieczeństwa,
- zapisywać w plikach samej Alfy (`%APPDATA%\Alfa`, `%LOCALAPPDATA%\Alfa`) ani usuwać katalogów systemu,
- czytać miejsc z hasłami i tokenami: `.ssh`, `.aws`, `.gnupg`, tokeny narzędzi CLI (`.claude`, `.codex`), profile
  i ciasteczka przeglądarek, Menedżer poświadczeń,
- sterować oknami Alfy, okna Brokera i watchdoga,
- obsługiwać strony i aplikacje dostawców AI (np. `claude.ai`) w Twoim imieniu.

Zawsze, na każdym poziomie, Alfa pyta, gdy: zleciłeś głosem coś nieodwracalnego (potwierdzasz kliknięciem, nie głosem)
albo agentka chce wysłać dane na zewnątrz po przeczytaniu niezaufanej treści.

## Okno Brokera

Zatwierdzenia nigdy nie dzieją się w oknie rozmowy ani w powiadomieniu. Służy do tego osobne, małe okno Brokera,
którego programy (także agentki) nie potrafią „kliknąć” za Ciebie: `Enter` niczego nie zatwierdza (`Tab` i `Spacja`
albo kliknięcie), `Esc` oznacza odmowę. W rozmowie widzisz kartę „czeka na zatwierdzenie” z opisem: co, dlaczego,
czy da się cofnąć, poziom ryzyka.

Przycisk na karcie tylko przenosi Cię do okna Brokera — zatwierdzasz zawsze tam. Okno Brokera uruchamia sam Broker.
Stan Brokera widzisz w **Ustawienia → Uprawnienia i bezpieczeństwo**:

- **Usługa Brokera** (osobne konto Windows) — pełna izolacja: okna Brokera nie „kliknie” żaden zwykły program.
  Usługę instaluje raz administrator komputera (jednorazowe potwierdzenie UAC) wg instrukcji wydania.
- **Tryb przenośny** — Broker działa jako proces Alfy na Twoim koncie. Wszystko działa tak samo, ale izolacja jest
  słabsza (Audyt i okno Brokera bez ochrony osobnego konta) — Alfa oznacza to wprost.
- **Połączenie zerwane / brak Brokera** — czerwony baner w rozmowie. To **bezpieczny stan**: wszystko, co wymaga zgody,
  jest odrzucane, a karta w rozmowie mówi, że prośba zostanie odrzucona. Alfa sama ponawia połączenie.

## STOP WSZYSTKIEGO (kill-switch)

`Ctrl+Shift+F12` — działa w całym systemie, także gdy okno Alfy jest ukryte albo nie odpowiada: skrót obsługuje
osobny proces **watchdog**, nie okno aplikacji. Gdy watchdog nie działa, skrót przejmuje awaryjnie Alfa i pokazuje
to na banerze. To samo robi „STOP WSZYSTKIEGO” w zasobniku i komenda głosowa „stop wszystko”. W ułamku sekundy:

- milknie mowa, zatrzymują się odpowiedzi i pobieranie modeli,
- zatrzymują się zadania agentek, a uruchomione przez nie programy zostają zamknięte,
- wszystkie wydane zgody tracą ważność; zdarzenie trafia do Audytu.

Tego skrótu nie można zmienić ani wyłączyć. Zwykłe „Stop” w kapsule aktywności zatrzymuje tylko bieżące zadanie.

## Cofanie

- Pliki usuwane przez agentkę trafiają do **Kosza**. Trwałe usunięcie wymaga potwierdzenia w oknie Brokera
  (w tej wersji niedostępne).
- Po każdym działaniu, które da się odwrócić, pojawia się toast **Cofnij** (8 s), a pod odpowiedzią karta z krokami
  do cofnięcia. Starsze kroki cofniesz w **Oś czasu → Replay → Cofnij krok**.
- Zanim agentka uruchomi polecenie powłoki, Alfa robi kopię katalogu roboczego — zmiany polecenia też da się cofnąć.
- Zapis do schowka wykonany przez agentkę można cofnąć.

## Sterowanie ekranem

Agentka z rolą **Wykonawczyni** może obsługiwać okna aplikacji: czytać ich strukturę, klikać, pisać, robić zrzuty.

- **Panel Ekran** (`Alt+5`): kto steruje i czym, lista akcji (bez wpisywanego tekstu), ostatni zrzut ekranu agentki.
  Na zrzucie zamaskowane są okna Alfy i Brokera, menedżery haseł i pola haseł; zrzut jest tylko w pamięci.
- **Zatrzymaj sterowanie** przerywa akcje agentek; **Oddaj sterowanie** pozwala im wrócić. Ruch myszy lub klawiatura
  w trakcie działania agentki też ją przerywa — Twoje wejście ma pierwszeństwo.
- W pasku tytułu widać „Delta steruje ekranem” z przyciskiem „Zatrzymaj”.
- Okna Alfy są niewidoczne na zrzutach ekranu.
- **Ustawienia → Komputer**: zasady, przejście do panelu Ekran, prośba „Zezwól na podgląd pulpitu” (24 h, przez okno
  Brokera — w tej wersji niedostępna).
- Okna programów uruchomionych jako administrator — **jeszcze niedostępne** (wymagają osobnego pomocnika).

## Twoje sekrety

Klucze API są tylko w Menedżerze poświadczeń Windows. Alfa nigdy nie czyta ani nie przechowuje tokenów narzędzi CLI
(Claude Code, Codex) ani ciasteczek przeglądarek. Treść z zewnątrz (strona, plik, mail, wynik narzędzia) jest
oznaczana jako niezaufana i agentki traktują ją jako dane, a nie polecenia.
