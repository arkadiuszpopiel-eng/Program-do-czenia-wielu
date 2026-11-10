# Lokalny model główny i lekka pomoc chmury — #67

8–12B jest szczeblem standardowym. W automatycznej pracy z profilem lokalnym8–12B zwykłe
zadania zaczynają się od niego, potem próbują znanych modeli lekkiej chmury: Luna,Haiku
lub odpowiedników. Dostawca musi być zarejestrowany/skonfigurowany; brak konta nie tworzy
fikcyjnego API. Dotyczy też braku pobranego modelu lokalnego, gdy jego domyślny model jestNone.

Trudne zadania i odrzucenie przez Krytyczkę zachowują dotychczasową eskalację. Jawne
kolejki, przypięcie i wyłączenie local_first zachowują ustawienia właściciela. Dobór nie
omija oceny zgodności/prywatności, uprawnień narzędzi, możliwości, zdrowia i budżetu.
Bielik1.5B local-light pozostaje w lekkiej klasie Summarize, poza główną pracą.

Genuine RED po poprawieniu fixture prywatności:3 zaliczone/3 błędy. Dodatkowy RED braku
modelu domyślnego0/1: wybieranoOpus zamiast dostępnejHaiku. Pełne router-contract/impl/fake
po naprawie64/0/0,16 wyników celów. Clippyall-targets/all-features kod0. PierwszyClippy
odrzucił nieużywany import; poprawiono go bez allow. Pełny zestaw obejmuje też wcześniejsze
regresje anulowania, hybrydy i zakazu odtwarzania rozpoczętego narzędzia na innym modelu.

Testy decyzji są odtwarzalne dla Alfy,Bety,Gamy,Delty i własnejLuny; sprawdzono też tryb
bez dostępnegoAPI, blokadę niedozwolonej chmury i budżetu. Nie są testem semantyki odpowiedzi
tych modeli, prawdziwegoAPI lubGPU. #41/#48 pozostają otwarte.
[Walidacja](artifacts/main-local-routing-validation.json). Bez nowych zgód lub rozszerzania
uprawnień i bez zmian progów ewaluacji. Nocne harmonogramy pozostają wyłączone.

Dodatkowy RED0/1: przy samejLunie sprawdzenie wybierało ponownie lokalny8B. Teraz lekka
chmura może sprawdzić wynik (review), gdy istniejące bramki na to pozwalają. Ręczny mocny
szczebel sprawdzania i kolejki nadal mają pierwszeństwo. Ponowne pełne64/0/0 i Clippy0.

Jawny lekki szczebel właściciela jest respektowany także dla firmowego aliasu API bez
nazwy Luna/Haiku. RED0/1 wybierał ponownie lokalny model; po poprawce pełne65/0/0
i Clippy0. Ranking nadal respektuje mocny szczebel i nie tworzy nieznanych dostawców.

PełneCI178 wykryło regresję dwóch niezmienionych testówCore: profil główny widoczny
w katalogu, lecz niepobrany, promował lekki model ponad średni w ręcznej kolejce konta
z włączonym przestawianiem. NowyRouterRED0/1 odtworzył dokładnie tę kolejność.
Preferencja pomocy obejmuje teraz kolejki automatyczne; ręczne przestawiane kolejki nadal
korzystają z dotychczasowej hierarchii. PełnyRouter66/0/0 i Clippy0. TestówCore nie zmieniono.

Dwa oryginalne testyCore model_routing po poprawce:2/0/0. SHA256 pliku testów jest
identyczny jak w niezaliczonymCI178. Końcowe lint/format/rozmiary/102 stronyHTML kod0.
[Dowody kolejnych regresji](artifacts/main-local-routing-followup.json).
