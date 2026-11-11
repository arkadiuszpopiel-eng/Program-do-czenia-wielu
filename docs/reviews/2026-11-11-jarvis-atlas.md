# Jarvis Atlas i cele dotykowe — 11.11.2026

**Kod: `2f3cf3e39a707ecb7c7a2ecee14772e37448fc88`.** Opcjonalny kokpit ma nowy,
leniwie ładowany widok „Mapa i systemy”: lokalny zarys świata, profil modeli, stan
sieci, konfigurację kont API, łącze Brokera, ostatni znany stan zdrowia i obsadę rozmowy.
Skróty otwierają istniejące ustawienia modeli, diagnostykę i panel agentek.

Nie zmieniono person, pięciu trybów zgód ani uprawnień. „Skonfigurowano konto” nie
oznacza udanego wywołania API; preferowany profil nie jest potwierdzeniem wykonania.
Nieznane zdrowie pozostaje nieznane. Telefon nadal nie jest połączony.

## Interfejs i dwie wykryte usterki

Audyt przed zmianą obejmował 1280×900,390×844,320×568. Obejrzano zapisane zrzuty,
także po zmianie. Rozbudowany widok otwiera się osobno, aby nie odbierać miejsca
rozmowie na telefonie; ma przewijanie, dostępny powrót, obsługę Escape i przywrócenie focusu.

Nowe sprawdzenia wykryły nadpisanie CSS przez style komponentu Button:
przyciski podglądu telefonu miały **32px zamiast44px**, a dodatkowy przycisk w nagłówku
Atlasu pozostawał na małym ekranie i ściskał tekst. Oba przypadki odtworzono przed
poprawką. Zwiększono swoistość selektorów lokalnych paneli; bez zmiany globalnych tokenów.
Testy sprawdzają teraz również rozmiary kontrolek wewnątrz obu paneli.

## Mapa i zasoby

Zarys lądów Natural Earth5.1.2 jest w domenie publicznej: lokalny SVG **28254bajty**,
bez sieci, fontów webowych, dodatkowego silnika, geolokalizacji, granic państw lub
wymyślonych tras. Mapa stanowi tło referencyjne, karty pokazują osobne dane aplikacji.
Nie ogłaszamy pomiaru pamięci WebView2 na podstawie wielkości pliku.
[Pochodzenie, hash i metoda](../../scripts/data/NATURAL-EARTH.md), generator
`node scripts/cockpit-map.mjs`; lint sprawdza hash źródła i identyczność wyniku.

## Dokładne wyniki

| Kontrola | Wynik |
|---|---|
| Pierwszy test nowego widoku na bazie |0zaliczonych/1niezaliczony: brak przycisku, kod1 |
| Pierwsza seria41,2workery |2timeouty axe po60s,2przerwane,37niewykonanych; zatrzymano własny proces, kod130 |
| Izolacja tego samego kodu,1worker |11/11 nowych testów,2,9min; bez zwiększenia limitów |
| Dotychczasowy kokpit i telefon przed poprawką CSS |30/30,3,4min; nie obejmowały jeszcze nowej asercji44px |
| Nowa regresja nagłówka |0/1: przycisk widoczny na320px |
| Nowa regresja przycisków telefonu |0/1: otrzymano32px zamiast44px |
| Końcowa wspólna seria po poprawkach |**41/41**,1worker,4,4min, kod0 |
| Vitest |**573/573**,103pliki, kod0 |
| Jarvis, własna persona, głos, autonomia i routing —8integracji Core |**19/0/0**,kod0; źródła Rustbfd69b97 identyczne w nowymUI, atrapy portów |
| svelte-check |**0błędów/0ostrzeżeń**, kod0 |
| Build, lint, format właściwych plików, mapa i diff |kod0 |

Końcowe41 obejmuje nowy Atlas11, dotychczasowy kokpit22 i podgląd telefonu8.
Nie dodajemy wcześniejszych serii do liczby unikalnych testów. Axe sprawdza istniejące
kryterium0critical/serious, PL/EN, light/dark,4style kokpitu,320/390/1280px i niski
ekran844×390, klawiaturę, focus, reduced-motion, dotykowe wymiary i brak zewnętrznych
żądań Atlasu. Podgląd telefonu nadal ma0WebSocket oraz zablokowane parowanie/polecenia.
Nie jest to pełny audyt WCAG ani sprzętowy odbiór telefonu.

Zachowano także początkowy błąd komendy formatera: `.prettierignore` nie ma parsera,
kod2; poprawiona komenda sprawdziła właściwe pliki z kodem0. Nie jest to błąd produktu.
Timeouty równoległego axe pozostają w danych; nie uznajemy ich za zaliczone lub
za usunięte jedynie dlatego, że seria z jednym workerem przeszła.

## Dowody i ograniczenia

[Surowe logi, polecenia i hashe](artifacts/2026-11-11-cloud/README.md).
[Desktop](artifacts/2026-11-11-atlas/atlas-1280x900.png) ·
[Telefon390px](artifacts/2026-11-11-atlas/atlas-390x844.png) ·
[Mały ekran320px](artifacts/2026-11-11-atlas/atlas-320x568.png).
To **fake IPC, headless Linux/Chromium**, bez prawdziwego telefonu, GPU, mikrofonu lub API.

Baza `bfd69b97` ma pełne CI18810/10 i Windows1114/4; sprawdzono ich zakończenie przed
publikacją nowego przyrostu. To wynik poprzednika. Nowy head wymaga własnego pełnego CI;
bieżące wyniki są przypisywane do commitów w [PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24).
Nie scalono PR i nie przeprowadzono niezależnego przeglądu.

#68 pozostaje otwarte dla odbioru fizycznego telefonu, #69 dla rzeczywistego transportu
i zgód. [Pakiet projektu #69](2026-11-11-remote-design-review.md) jest do przeglądu,
nie dowodem działającego sterowania. [#58 nadal niezaliczone](2026-11-11-ledger-diagnosis.md).
