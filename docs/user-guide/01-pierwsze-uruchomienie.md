# Pierwsze uruchomienie i wprowadzenie

## Czego potrzebujesz

- Windows 11 (64-bitowy). Środowisko WebView2 jest w systemie.
- Około 3 GB wolnego miejsca na model lokalny (więcej, jeśli dodasz modele głosu).
- Mikrofon i głośniki lub słuchawki — tylko do rozmowy głosowej.
- Klucze API nie są potrzebne.

## Instalacja

Alfa nie ma jeszcze publicznego wydania: instalator `Alfa_*.exe` (z aktualizacjami i powrotem do poprzedniej wersji)
budujesz ze źródeł. Najprościej skryptem `scripts\setup-dev.ps1` — krok po kroku w
[Pierwszy test na Twoim PC](11-pierwszy-test-na-pc.md). Automatyczne aktualizacje działają dopiero z wydania
podpisanego Twoim kluczem (`docs/RELEASE.md`).

## Wprowadzenie (8 kroków)

Przy pierwszym starcie Alfa prowadzi Cię przez krótkie wprowadzenie. Każdy krok można pominąć i wrócić do niego
później: paleta poleceń `Ctrl+K` → „Uruchom ponownie wprowadzenie”.

1. **Mikrofon** — wybierz urządzenie i powiedz kilka słów; pasek poziomu powinien się poruszać.
2. **Profil głosu** — Alfa proponuje profil dopasowany do komputera: A (najlżejszy, sam procesor), B, C lub D
   (najlepsza jakość, mocna karta graficzna).
3. **Sprzęt** — pomiar procesora, pamięci i karty graficznej. Na tej podstawie Alfa dobiera modele.
4. **Konta i klucze** — „Dodaj teraz” albo „Pomiń — dodam później”. W tym kroku jest też karta **Model lokalny**:
   pobierz go, żeby Alfa działała bez kluczy i bez internetu (zobacz [Konta i model lokalny](02-konta-i-model-lokalny.md)).
5. **Autonomia** — krótki opis poziomów L0–L4. Startujesz na L3 (zobacz [Bezpieczeństwo](07-bezpieczenstwo.md)).
6. **Korpus głosu** (opcjonalnie) — nagranie Twojego głosu do lepszego rozpoznawania mowy. Samo nagrywanie jest
   **jeszcze niedostępne**; wybierz „Później”.
7. **Mosty CLI** (opcjonalnie) — jeśli używasz Claude Code albo Codex, możesz się zalogować we wbudowanym terminalu
   (zobacz [Mosty CLI](08-mosty-cli.md)).
8. **Import** (opcjonalnie) — przeniesienie konfiguracji i sesji z innego Twojego komputera z pliku `.alfa`.

Na końcu kliknij **Zaczynamy**.

## Okno główne

- **Pasek tytułu** w trzech strefach — po lewej przełącznik panelu Sesje i nazwa sesji (kliknij albo `F2`, żeby
  zmienić); na środku awatary agentek α β γ δ (ta, która mówi, porusza ustami, a z ust wylatują litery właśnie
  pisanej odpowiedzi); po prawej wskaźniki na żywo (sterowanie ekranem, kamera, mikrofon), przełącznik panelu
  prawego i przycisk ustawień. Gdy włączona jest **Pełna automatyka**, świeci tu też bursztynowy znak trybu
  (kliknij — otworzy menu trybów).
- **Panel Sesje** po lewej — lista rozmów (`Ctrl+B` albo przycisk w nagłówku panelu chowa go w całości).
- **Panele chowane:** schowany panel znika cały, a przy krawędzi okna zostaje cienki świecący znacznik; najechanie
  wysuwa panel nad rozmowę, zjechanie myszą, kliknięcie obok albo `Esc` go chowa, a kliknięcie znacznika albo
  **Przypnij panel** dokuje go z powrotem. Z klawiatury: `Tab` do znacznika i `Enter`. Oba panele chowają się
  same od pierwszego uruchomienia; stały układ przywraca **Przypnij panel**, przełącznik **Wygląd → Panele
  chowają się same** albo polecenie palety „Chowaj panele”. W Ustawieniach tak
  samo chowa się menu stron (przycisk obok tytułu „Ustawienia” albo `Ctrl+B`).
- **Rozmowa** na środku i pole wiadomości na dole. W pasku pisania obok Pipeline'ów i trybu Auto są żetony
  stanu: profil modeli z kropką połączenia (kliknij — profil zmienisz od razu), **Tryb pracy** (menu z czterema
  trybami, zobacz [Bezpieczeństwo](07-bezpieczenstwo.md)) i koszt sesji.
- **Panel prawy** (`Ctrl+\`) z kartami: Agentki (`Alt+1`), Oś czasu (`Alt+2`), Pliki (`Alt+3`), Pamięć (`Alt+4`),
  Ekran (`Alt+5`), Głos (`Alt+6` — **jeszcze niedostępny**, ustawienia głosu są w Ustawieniach) i Zadania (`Alt+7`).
- **Tryb skupienia** (`F11`) chowa wszystko poza rozmową; `Esc` albo `F11` przywraca widok. Tryb zostaje
  włączony po ponownym uruchomieniu.
- **Instrukcja** (`F1`) — ten przewodnik w programie; wyszukiwanie w Ustawieniach podpowiada też fragmenty
  instrukcji.
- **Paleta poleceń** (`Ctrl+K`) — wpisz kilka liter polecenia, nazwy sesji albo ustawienia.
- W wąskim oknie panele zamieniają się w wysuwane arkusze (układ sprawdzany od 360 px szerokości).

## Ustawienia

`Ctrl+,` albo ikona w pasku tytułu. Po lewej drzewo sekcji, u góry wyszukiwarka. Przy każdym ustawieniu jest opis,
wartość domyślna i przycisk „Przywróć domyślne”. W **Ogólne → Profil widoku ustawień** wybierzesz, ile opcji widać
(Prosty, Zaawansowany, Ekspert). Język interfejsu (polski / angielski) zmienisz w **Język**, motyw jasny, ciemny lub
automatyczny — w **Wygląd**. Tam też **Styl**: Klasyczny, Szkło, Next albo **Junior (dla dzieci)** — cały interfejs
o 15 % większy, ciepłe kolory i zaokrąglone kształty (za dnia krem z fioletem, w nocy śliwka ze słonecznym akcentem).
Po wybraniu Juniora Alfa proponuje jednym przyciskiem **Włącz** wolniejszą mowę (85 %), dźwięki sygnałowe i
**Uproszczony ekran**; bez kliknięcia nic się nie zmienia, a każdą z tych rzeczy ustawisz potem osobno w **Głos**,
**Powiadomienia** i **Wygląd**. Uproszczony ekran chowa przełączniki paneli, Pipeline'y, wybór agentki i rzadsze
akcje wiadomości (zostają „Kopiuj” i „Przeczytaj na głos”); wskaźniki mikrofonu, kamery i sterowania oraz tryb
pracy widać zawsze. To wygoda, nie blokada — skróty klawiszowe działają dalej. Gdy Alfa czyta odpowiedź na głos
(przyciskiem albo w rozmowie głosowej), zdanie, które właśnie słychać, jest wyróżnione w treści wiadomości.

## Zasobnik systemowy

Zamknięcie okna chowa Alfę do zasobnika (zmienisz to w **Ogólne → Zamknięcie okna chowa do zasobnika**). Po 10
minutach ukrycia okno zwalnia pamięć i odtwarza się przy ponownym otwarciu. Menu ikony w zasobniku:

- Pokaż Alfę, Nowa rozmowa, Szybkie pytanie,
- Głos wł./wył., Nie przeszkadzać (wycisza powiadomienia),
- **STOP WSZYSTKIEGO** — to samo co `Ctrl+Shift+F12`,
- Wyjście.

## Szybkie pytanie

`Ctrl+Alt+Space` otwiera małe okno na środku ekranu. Wpisz pytanie i naciśnij `Enter` — odpowiedź pojawi się pod
spodem. Po odpowiedzi `Enter` w pustym polu otwiera rozmowę w pełnym oknie, `Esc` zamyka okienko. W **Ogólne →
Szybkie pytanie: sesja** wybierzesz, czy pytania trafiają do jednej wspólnej sesji, czy każde tworzy nową.

## Komunikaty u góry rozmowy

- **Brak połączenia** — działa model lokalny, a wiadomości do chmury czekają w kolejce („Ponów teraz”).
- **Brak kluczy** — działa profil lokalny; przycisk „Dodaj klucz” otwiera kreator.
- **Limit zapytań u dostawcy** — Alfa podaje, kiedy limit się odnowi, i kieruje zadania do innych modeli.
- **Windows blokuje mikrofon** — przycisk otwiera ustawienia prywatności mikrofonu.
- **Mało miejsca na dysku** — przycisk otwiera Czujnik miejsca.
