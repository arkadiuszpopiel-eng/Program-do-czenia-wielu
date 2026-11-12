# 57 — role modeli: lokalny 27B do kodu, mniejsze do rozmowy, pomoc chmury

**Decyzja właściciela z 12.11.2026.** Kodowanie i trudne zadania lokalne mają
korzystać wyłącznie z modelu klasy **Qwen3.8-27B Q4**. Słabsze modele służą agentkom
lokalnie do rozmowy, wyszukiwania informacji i prostszej nauki. Skonfigurowane modele
chmurowe klasy **Luna/Haiku i mocniejsze** pomagają przy kodzie, wyszukiwaniu i
narzędziach, gdy model lokalny nie daje rady. Dalsza eskalacja zależy od trudności
i dotychczasowej hierarchii. Decyzja zastępuje przypisywanie 8–12B głównego kodowania
z wcześniejszego planu55; nie odbiera małym modelom roli rozmówczyń.

**Stan: wybór ról zapisany, runtime jeszcze nie przełączony.** Nie zainstalowano
27B, nie zmieniono produkcyjnych ustawień, progów evals ani polityk bezpieczeństwa.
Nie uznajemy liczby parametrów za wynik testu kodowania. Poprzednie błędne odpowiedzi
8B/11B/14B pozostają w [pełnej diagnozie](../reviews/2026-11-12-cpu-extended-models.md).

## Macierz pracy wszystkich person

| Zadanie | Lokalnie | Pomoc chmury |
|---|---|---|
| Rozmowa, objaśnienie, proste streszczenie | Mniejszy, sprawdzony model; Bielik 1.5B jako lekki pomocnik | Dostępny Luna/Haiku lub odpowiednik po problemie |
| Proste wyszukiwanie i odczyt WWW | Mniejszy model z rzeczywistą obsługą potrzebnych narzędzi | Luna/Haiku lub mocniejszy przy problemie narzędzia, źródeł lub analizy |
| Kod, debugowanie, kompilacja, testy, poprawki | Zakwalifikowany Qwen3.8-27B Q4; bez zastępowania go małym modelem | Dostępny i dopuszczony Luna/Haiku lub mocniejszy |
| Trudne rozumowanie, architektura i trudna analiza kodu | Ta sama klasa lokalnego 27B, z odpowiednim budżetem kontekstu/rozumowania | Mocniejszy szczebel stosownie do zadania; po odrzuceniu kolejna kwalifikowana trasa |

Macierz dotyczy Alfy, Bety, Gamy, Delty i własnych person; osobowość i nazwa agentki
nie są nazwą silnika. „Luna” jako profil modelu chmurowego nie jest własną personą
Luna. Konkretny identyfikator dostawcy/modelu wybieramy z działającego konta i katalogu,
nie tworzymy go przez samo dopisanie nazwy. Brak konta oznacza brak tej trasy.

Proste WWW obejmuje rzeczywiste wywołanie i odczyt źródeł. Sukces odpowiedzi modelu
nie dowodzi wykonania narzędzia lub jakości wyszukiwarki. Złożone zadanie badawcze
może wymagać mocniejszego modelu. Obecny mapujący role kod traktuje `research` jako
Planning: wdrożenie musi oddzielić proste wyszukiwanie od trudnego planowania.

## Sprawdzony kandydat i granica sprzętu

[Oficjalny Qwen3.8-27B](https://huggingface.co/Qwen/Qwen3.8-27B) istnieje.
Metadane producenta wskazują 27781427952 parametrów, Apache2.0, architekturę
Qwen3_5ForConditionalGeneration. Rewizja źródłowa:
`37561816e66b5b9153b1b64c10968c18c1e6f4a0`.

Kwantyzacja jest osobnym produktem społecznościowym:
[Unsloth/Qwen3.8-27B-GGUF](https://huggingface.co/unsloth/Qwen3.8-27B-GGUF),
rewizja `4af51b3542f775b0a8e4b310bfb3f17e01c306c9`.

| Wariant | Bajty samych wag | GiB samych wag |
|---|---|---|
| Q4_0 | 16056478688 | 14.95 |
| UD-Q4_K_M | 16564259744 | 15.43 |

**Żaden z tych wariantów nie mieści się w całości w RTX 2060 12 GB**, nawet przed
kontekstem, buforami, pulpitem i mową. Pełne GPU wymaga większej pamięci; karta 24 GB
jest kandydatem do pomiaru, nie gwarancją dowolnego kontekstu. Częściowy offload
GPU+RAM jest oddzielną możliwością, wymagającą pomiaru dostępnego RAM, opóźnienia
i przepustowości; nie obiecujemy płynności lub szybkości pełnego GPU. Wariant
sprzętowy pozostaje do odpowiedzi właściciela; samo wybranie modelu nie zmienia 12 GB.

Cloud tej sesji ma 16 GiB RAM i nie ma GPU. Nie pobieramy tutaj 16,56 GB wag w celu
pozorowania testu na docelowej karcie. [Zapis publicznych metadanych i hashe
ich kopii](../reviews/artifacts/2026-11-12-model-roles/manifest.json) zawiera piny,
rozmiary i deklarowane SHA256 LFS. **Nie są one lokalną weryfikacją pełnych wag**.
Nie wykonano inferencji 27B, testu CUDA, rzeczywistego API lub pomiaru zapasu kontekstu.

## Konkretne zmiany wymagane w routerze

Obecny `router-impl/src/main_local.rs` preferuje 8–12B także dla Code/Planning;
`order_by_rung` zostawia niższe szczeble jako zapas. Ustawienie początku kolejki
na 27B samo nie spełni nowej zasady. Implementacja ma:

1. Oddzielić **dopuszczalność roli** od rankingu i kosztu. Do kodu/trudnego zadania
   nie wystawiać lokalnych 1.5B/8B/11B/14B jako zapasu; jawnie obsłużyć ręczne przypięcia,
   wyłączony dobór trudności, ponowienie, Krytyczkę, wznowienie i delegację do innej persony.
   Niedozwolone przypięcie ma dać czytelny powód lub wymagać jawnej zmiany profilu,
   bez cichego wykonania na słabszym silniku.
2. Powiązać lokalny profil kodowania z konkretnym zweryfikowanym modelem/runtime
   i pomiarem sprzętu, zamiast samą liczbą „27B” w nazwie. Nieznana zdolność nie
   jest potwierdzoną obsługą funkcji. Sprawdzić format tool calls i thinking w przypiętym llama.cpp.
3. Przy niedostępnym 27B, za małej pamięci/kontekście, błędzie lub odrzuceniu jakości
   wybierać wyłącznie dopuszczoną, skonfigurowaną trasę chmurową odpowiedniego szczebla.
   Jeśli jej brak lub sesja zabrania chmury — jasno zatrzymać zadanie, zachowując rozmowę.
4. Zachować prywatność/local_only, budżet, możliwości narzędzi i dotychczasowe zgody.
   Wybór mocniejszego modelu nie nadaje wyższych uprawnień. Po rozpoczęciu narzędzia
   lub zapisie nie ponawiać operacji automatycznie na innym silniku.
5. W UI pokazać agentkę, rzeczywisty model, powód zmiany, źródło odpowiedzi,
   tryb GPU/offload i ograniczenia zasobów. Brak zasobów nie jest stanem „gotowe”.

## Testy przed odbiorem

| Kontrola | Oczekiwany dowód |
|---|---|
| Kod/trudne zadanie z dostępnym 27B | Rzeczywista kwalifikowana trasa lokalna, małe modele bez wywołań |
| Brak 27B / za mały kontekst / błąd przed wyjściem | Dopuszczona chmura; brak powrotu do lokalnego 8B; historia zachowana |
| Brak 27B i brak konta / prywatne / budżet zablokowany | Czytelny brak trasy, bez wysyłki lub fałszywego sukcesu |
| Ręczne przypięcie 8B, kolejki, routing off, review, retry | Brak obejścia ograniczenia roli |
| Mały model w rozmowie/prostym WWW | Nadal działa, z obsługą rzeczywistych narzędzi i oceną źródeł |
| Macierz person/trybów | Ta sama zasada dla każdej persony i trybu |
| Prawdziwy odczyt / pusta sesja / same metadane / odmowa / Undo | Poprawny opis działań; zachowane wszystkie surowe odpowiedzi |
| Kodowanie od początku do końca | Odczyt → zmiana → kompilacja → test → przegląd → poprawka, bez deklaracji bez dowodu |
| GPU 12 GB + RAM lub większa karta | VRAM/RAM, kontekst, zapas na mowę, opóźnienia, Stop i błędy; osobno od fake/Linux |
| API i narzędzia | Rzeczywisty uprawniony dostawca, bez sekretów w logach; retry bez powtórnych efektów |

Kolejność: wybór miejsca uruchomienia 27B i dostępnych profili API → regresje doboru
na kontrolowanych portach → poprawka routera i integracji → zgodność runtime/hash
→ próby 27B na właściwym sprzęcie oraz rzeczywista chmura → niezależny przegląd/CI.
Nie rozszerzamy wcześniejszych zgód na chronione moduły. Nie zamykamy #41/#48/#58/#77
na podstawie planu, samej liczby parametrów, zielonego transportu lub wcześniejszego CI.
