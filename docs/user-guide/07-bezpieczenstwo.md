# Bezpieczeństwo: poziomy, Sędzia Cieni, kill-switch, cofanie

Nad wszystkim, co robią agentki, czuwa **Sędzia Cieni** — część Alfy, której agentki nie mogą zmienić. Każde działanie
(odczyt pliku, polecenie, kliknięcie w oknie) dostaje od niego jednorazową zgodę albo odmowę, a decyzja trafia do
dziennika Audytu, którego nie da się po cichu poprawić.

## Tryb pracy (poziomy autonomii L0–L4)

„Jak bardzo agentka działa sama”. Domyślnie **Automatycznie (L3)**.

| Tryb | Co to znaczy |
| --- | --- |
| Plan (L0) | Agentka czyta i przygotowuje plan, bez zmieniania plików i systemu. Plan dotyczy bieżącej rozmowy. |
| Ręcznie (L1) | Każda zmiana wymaga Twojej zgody. |
| Akceptuj edycje (L2) | Edycje w katalogu roboczym wykonuje bez osobnego pytania; inne działania nadal ocenia Sędzia Cieni. |
| Automatycznie (L3, domyślny) | Zwykłe działania w dozwolonym zakresie wykonuje sama. Przy prośbie o zgodę model-sędzia może ocenić ją w granicach reguł; decyzje zastrzeżone dla właściciela trafiają do Ciebie. |
| Pełny dostęp (L4) | Działa bez pytań w zakresie twardych blokad Jądra. Pozostają STOP i dziennik. Podniesienie poziomu zatwierdzasz w osobnym oknie Sędziego. |

**Menu „Tryb pracy” w pasku pisania**: pięć trybów, klawisze `1`–`5`, `Esc` zamyka menu.
Wybierz **Ta rozmowa** lub **Wszystkie rozmowy** przed zmianą. Plan, wyjście z Planu oraz odblokowanie
„do końca rozmowy” zachowują zakres bieżącej rozmowy. Menu pokazuje poziom wybranego zakresu.
`Alt+M` przechodzi do następnego trybu rozmowy; skrót nie włącza Pełnego dostępu.

To samo menu otworzysz z palety (`Ctrl+K` → „Tryb pracy”). Pełne ustawienia są w **Ustawienia →
Uprawnienia i bezpieczeństwo**. Obniżenie działa od razu. Podniesienie potwierdzasz w oknie Sędziego Cieni;
komunikat o zatwierdzeniu pojawia się po odczytaniu jego stanu. Agentka sama nie podnosi poziomu.
Profil współpracy **Jarvis** jest osobnym wyborem w panelu Agentki i zachowuje ten tryb zgód.

## Czego nie da się zrobić na żadnym poziomie (także L4)

- wyłączyć ani zmienić Audytu, watchdoga, kill-switcha i zasad bezpieczeństwa,
- zapisywać w plikach samej Alfy (`%APPDATA%\Alfa`, `%LOCALAPPDATA%\Alfa`) ani usuwać katalogów systemu,
- czytać miejsc z hasłami i tokenami: `.ssh`, `.aws`, `.gnupg`, tokeny narzędzi CLI (`.claude`, `.codex`), profile
  i ciasteczka przeglądarek, Menedżer poświadczeń,
- sterować oknami Alfy, okna Sędziego Cieni i watchdoga,
- obsługiwać strony i aplikacje dostawców AI (np. `claude.ai`) w Twoim imieniu.

Na poziomach L0–L3 Alfa pyta zawsze, gdy: zleciłeś głosem coś nieodwracalnego (potwierdzasz kliknięciem, nie głosem)
albo agentka chce wysłać dane na zewnątrz po przeczytaniu niezaufanej treści. Na „Pełnej automatyce” (L4) nie pyta
o nic — zostają tylko blokady z listy wyżej.

## Okno Sędziego Cieni

Zatwierdzenia nigdy nie dzieją się w oknie rozmowy ani w powiadomieniu. Służy do tego osobne, małe okno Sędziego Cieni,
którego programy (także agentki) nie potrafią „kliknąć” za Ciebie: `Enter` niczego nie zatwierdza (`Tab` i `Spacja`
albo kliknięcie), `Esc` oznacza odmowę. Na górze okna jest „?” w kółku (albo `F1`) — rozwija opis, czym jest Sędzia
Cieni i jak ma się do trybu pracy. Okno pokazuje najpierw, **co dokładnie** agentka chce zrobić (np. „trwałe usunięcie
pliku C:\…
aport.xlsx”), potem kto, gdzie, ryzyko, źródło polecenia i dlaczego pyta (z trybem pracy). Na „Pełnej
automatyce” okno się nie pokazuje. W rozmowie widzisz kartę „czeka na zatwierdzenie” z opisem: co, dlaczego,
czy da się cofnąć, poziom ryzyka.

### Zgody „Zawsze w tym zakresie”

Gdy to samo działanie powtarza się w tym samym miejscu (np. zapisy w jednym folderze projektu), w oknie Sędziego
Cieni możesz zamiast „Zezwól tylko teraz” wybrać w rzędzie **„Zawsze w tym zakresie:”** czas: **1 h**, **3 h**, **24 h** albo
**Do odwołania**. Do końca tego czasu Sędzia przepuszcza ten sam rodzaj działań w tym samym miejscu bez pytania —
nic więcej. Listę takich zgód (zakres, agentka, do kiedy) i przycisk **„Odwołaj”** znajdziesz w **Ustawienia →
Uprawnienia i bezpieczeństwo**; odwołanie działa od razu i nie wymaga okna Sędziego. Nowej zgody nie da się
tam dodać — tylko w oknie Sędziego. STOP WSZYSTKIEGO kasuje wszystkie zgody.

Przycisk na karcie tylko przenosi Cię do okna Sędziego Cieni — zatwierdzasz zawsze tam. Okno uruchamia sam Sędzia Cieni.
Stan Sędziego Cieni widzisz w **Ustawienia → Uprawnienia i bezpieczeństwo**:

- **Usługa Sędziego Cieni** (osobne konto Windows) — pełna izolacja: okna Sędziego Cieni nie „kliknie” żaden zwykły program.
  Usługę instaluje raz administrator komputera (jednorazowe potwierdzenie UAC) wg instrukcji wydania.
- **Tryb przenośny** — Sędzia Cieni działa jako proces Alfy na Twoim koncie. Wszystko działa tak samo, ale izolacja jest
  słabsza (Audyt i okno Sędziego Cieni bez ochrony osobnego konta) — Alfa oznacza to wprost.
- **Połączenie zerwane / brak Sędziego Cieni** — czerwony baner w rozmowie. To **bezpieczny stan**: wszystko, co wymaga zgody,
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

- Pliki usuwane przez agentkę trafiają do **Kosza**. Trwałe usunięcie wymaga potwierdzenia w oknie Sędziego Cieni
  (w tej wersji niedostępne).
- Po każdym działaniu, które da się odwrócić, pojawia się toast **Cofnij** (8 s), a pod odpowiedzią karta z krokami
  do cofnięcia. Starsze kroki cofniesz w **Oś czasu → Replay → Cofnij krok**.
- Zanim agentka uruchomi polecenie powłoki, Alfa robi kopię katalogu roboczego — zmiany polecenia też da się cofnąć.
- Zapis do schowka wykonany przez agentkę można cofnąć.

## Sterowanie ekranem

Agentka z rolą **Wykonawczyni** może obsługiwać okna aplikacji: czytać ich strukturę, klikać, pisać, robić zrzuty.

- **Panel Ekran** (`Alt+5`): kto steruje i czym, lista akcji (bez wpisywanego tekstu), ostatni zrzut ekranu agentki.
  Na zrzucie zamaskowane są okna Alfy i Sędziego Cieni, menedżery haseł i pola haseł; zrzut jest tylko w pamięci.
- **Zatrzymaj sterowanie** przerywa akcje agentek; **Oddaj sterowanie** pozwala im wrócić. Ruch myszy lub klawiatura
  w trakcie działania agentki też ją przerywa — Twoje wejście ma pierwszeństwo.
- W pasku tytułu widać „Delta steruje ekranem” z przyciskiem „Zatrzymaj”.
- Agentki nie widzą okien Alfy ani okna Sędziego Cieni — na ich zrzutach te okna są zawsze zamaskowane. Twoje zrzuty
  (Wycinek, `PrintScreen`) obejmują okna Alfy normalnie; jeśli chcesz je ukrywać (np. przy udostępnianiu ekranu),
  włącz **Ustawienia → Uprawnienia i bezpieczeństwo → Ukrywaj okna Alfy na zrzutach i nagraniach ekranu**
  (działa po ponownym uruchomieniu Alfy).
- **Ustawienia → Komputer**: zasady, przejście do panelu Ekran, prośba „Zezwól na podgląd pulpitu” (24 h, przez okno
  Sędziego Cieni — w tej wersji niedostępna).
- Okna programów uruchomionych jako administrator — **jeszcze niedostępne** (wymagają osobnego pomocnika).

## Gdzie trafiają dane

Ustawienia → Dobór modelu → **Gdzie trafiają dane** pokazuje każdą drogę do modelu w chmurze (klucz API albo most
CLI): w jakim kraju działa dostawca, czy może uczyć modele na twoich wiadomościach i czy obsłuży sesję prywatną —
a jeśli nie, dlaczego (np. Chiny albo nieznane zasady trenowania). Te reguły pochodzą z rejestru zgodności Alfy
i nie zmienia się ich w tym miejscu.

## Twoje sekrety

Klucze API są tylko w Menedżerze poświadczeń Windows. Alfa nigdy nie czyta ani nie przechowuje tokenów narzędzi CLI
(Claude Code, Codex) ani ciasteczek przeglądarek. Treść z zewnątrz (strona, plik, mail, wynik narzędzia) jest
oznaczana jako niezaufana i agentki traktują ją jako dane, a nie polecenia.
