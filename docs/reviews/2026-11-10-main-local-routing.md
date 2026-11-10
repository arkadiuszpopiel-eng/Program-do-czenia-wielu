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
