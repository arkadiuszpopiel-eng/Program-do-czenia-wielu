# Alfa — przewodnik użytkownika

Alfa to asystentka na Windows 11. Rozmawiasz z nią tekstem albo głosem, a cztery agentki — **Alfa, Beta, Gama
i Delta** — dzielą się pracą: prowadzą rozmowę, porządkują pliki, pilnują pamięci i sprawdzają wyniki. Alfa działa
bez żadnych kluczy, na modelu językowym uruchomionym na Twoim komputerze. Klucze do usług w chmurze możesz dodać
w każdej chwili.

## Stan tej wersji (październik 2026)

To wersja rozwojowa (0.0.1). Opisujemy tylko to, co jest w aplikacji. Tam, gdzie czegoś brakuje albo trzeba coś
przygotować samodzielnie, w tekście są oznaczenia:

| Oznaczenie              | Znaczenie                                                                                           |
| ----------------------- | --------------------------------------------------------------------------------------------------- |
| **Wymaga klucza**       | Potrzebny klucz API dostawcy (zobacz [Konta, klucze i model lokalny](02-konta-i-model-lokalny.md)). |
| **Wymaga plików**       | Program lub model trzeba na razie skopiować ręcznie do katalogu Alfy.                               |
| **Jeszcze niedostępne** | Funkcja jest zaplanowana albo gotowa „pod spodem”, ale aplikacja jej jeszcze nie pokazuje.          |

Najważniejsze ograniczenie: **okno Brokera (okno zatwierdzeń) nie jest jeszcze podłączone**. Działania agentek,
które wymagają Twojej zgody, kończą się odmową po upływie czasu, a poziomu autonomii nie da się podnieść — można go
tylko obniżyć. Szczegóły: [Bezpieczeństwo](07-bezpieczenstwo.md).

## Spis treści

1. [Pierwsze uruchomienie i wprowadzenie](01-pierwsze-uruchomienie.md)
2. [Konta, klucze i model lokalny](02-konta-i-model-lokalny.md)
3. [Rozmowa, gałęzie i sesje](03-rozmowa.md)
4. [Głos](04-glos.md)
5. [Agentki, obsady, zadania i Marszałek](05-agentki-i-zadania.md)
6. [Pamięć](06-pamiec.md)
7. [Bezpieczeństwo: poziomy, kill-switch, cofanie](07-bezpieczenstwo.md)
8. [Mosty CLI (Claude Code, Codex)](08-mosty-cli.md)
9. [Przenoszenie danych, kopie i Zdrowie systemu](09-dane-i-zdrowie.md)
10. [Modele i silniki](10-modele-i-silniki.md)

## Skróty klawiszowe

Wszystkie skróty poza oznaczonymi „stały” zmienisz w **Ustawienia → Skróty**. Pełną listę pokazuje `Ctrl+/`.

| Skrót                                        | Działanie                                                              |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `Ctrl+Shift+F12`                             | **STOP WSZYSTKIEGO** — działa w całym systemie (stały)                 |
| `Ctrl+Alt+Space`                             | Szybkie pytanie — działa w całym systemie                              |
| `Ctrl+N`                                     | Nowa rozmowa                                                           |
| `Ctrl+P`                                     | Szybkie przełączanie sesji                                             |
| `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+1…9`     | Następna, poprzednia, wybrana karta                                    |
| `Ctrl+W`, `Ctrl+Shift+T`                     | Zamknij kartę, przywróć zamkniętą                                      |
| `F2`                                         | Zmień nazwę sesji                                                      |
| `Ctrl+F`, `Ctrl+Shift+F`                     | Szukaj w rozmowie, szukaj we wszystkich sesjach                        |
| `Ctrl+K`                                     | Paleta poleceń                                                         |
| `Ctrl+/`                                     | Ściągawka skrótów                                                      |
| `Ctrl+,`                                     | Ustawienia                                                             |
| `F11` albo `Ctrl+Shift+Enter`                | Tryb skupienia (tylko rozmowa)                                         |
| `Ctrl+=`, `Ctrl+-`, `Ctrl+0`                 | Powiększ, pomniejsz, rozmiar domyślny                                  |
| `Ctrl+B`, `Ctrl+\`                           | Panel Sesje, panel prawy                                               |
| `Alt+1` … `Alt+7`                            | Panele: Agentki, Oś czasu, Pliki, Pamięć, Ekran, Głos, Zadania         |
| `Ctrl+Shift+M`                               | Mikrofon włącz/wyłącz                                                  |
| przytrzymana `Spacja` (poza polem tekstowym) | Mów, gdy w ustawieniach głosu wybrano „Przytrzymaj, aby mówić” (stały) |
| `Enter`, `Shift+Enter`                       | Wyślij, nowa linia (stały; zamianę włączysz w Ustawienia → Skróty)     |
| `↑` w pustym polu                            | Edytuj ostatnią wiadomość (stały)                                      |
| `Ctrl+↑`, `Ctrl+↓`                           | Poprzednia, następna wysłana wiadomość (stały)                         |
| `Ctrl+Shift+V`                               | Wklej jako zwykły tekst (stały)                                        |
| `Esc`                                        | Kolejno: zamknij menu → zatrzymaj mowę → zatrzymaj odpowiedź (stały)   |

Na polskiej klawiaturze `Ctrl+Alt` działa jak `AltGr`, dlatego Alfa nie pozwoli ustawić skrótu `Ctrl+Alt` (ani
`Ctrl+Alt+Shift`) z literami a, c, e, l, n, o, s, x, z — kolidowałby z wpisywaniem polskich znaków. Odświeżanie okna
(`F5`, `Ctrl+R`) jest wyłączone.

## Gdzie Alfa trzyma dane

| Katalog                    | Zawartość                                                                                                     |
| -------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `%APPDATA%\Alfa\config`    | Ustawienia (pliki TOML)                                                                                       |
| `%LOCALAPPDATA%\Alfa`      | Sesje (zaszyfrowane bazy), pamięć, modele (`models`), programy pomocnicze (`sidecars`), logi, migawki importu |
| `%USERPROFILE%\Alfa\Sesje` | Katalogi robocze sesji — tu agentki zapisują pliki                                                            |

Klucze API są wyłącznie w Menedżerze poświadczeń Windows — nigdy w tych katalogach.
