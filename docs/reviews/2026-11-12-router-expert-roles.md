# Kod i trudne zadania bez słabego lokalnego fallbacku

**Uzupełnienie po62f8e573:** [osobna korekta research](2026-11-12-research-code-query.md)
pozwala szukać dokumentacji zawierającej kod bez wymuszania roli programistycznej.
Poniższe137 kontroli dotyczą62f8e573; nowa korekta ma własny zapis81 testów routera.

Przyrost na bazie a836bb57 / PR88, zgodny z decyzją właściciela i
[planem 57](../plan/57-role-modeli-27b-i-pomoc-chmury.md). Dotyczy modułu routera;
nie instaluje modelu 27B, nie podnosi uprawnień i nie zmienia polityk bezpieczeństwa.

## Przyczyna i zachowanie po zmianie

Ranking według szczebli pozostawiał niższe modele na końcu kolejki. W efekcie Kod,
Planning, a nawet ręczne przypięcie słabego lokalnego modelu mogły skończyć na 8B.
Wyłączenie rankingu lub review nie dawało osobnego ograniczenia roli. Mały model
pozostawał także po krótkim „popraw”, gdy wcześniejszy profil był ekspercki.

Nowa bramka `ModelRole` jest niezależna od rankingu i filtruje całą listę tras.
W klasach Code/Planning oraz rozpoznanym kodzie/trudnym zadaniu lokalnie dopuszcza
wyłącznie konkretne profile `qwen3.8-27b-ud-q4-k-m` i `qwen3.8-27b-q4-0`, z wpisem
możliwości w rejestrze adaptera. Ręczny szczebel Strong, sama nazwa „70B” lub sama
nazwa 27B bez takiego wpisu nie wystarczą. Ten wybór identyfikatorów nie jest
poświadczeniem jakości: pełny hash, zgodność runtime i sprzęt nadal wymagają odbioru.

Słaby model odpada także z fallbacku, przypięcia i review oraz przy wyłączonym
rankingu. Jawne kolejki zachowują porządek dopuszczonych tras. W automatycznym
trybie local_first router odkrywa profil ekspercki także obok małego modelu
domyślnego. Eskalacja zachowuje istniejącą kolejność hierarchii. Wskazówka zadania
nie ukrywa kodu w bieżącym żądaniu; krótki follow-up po profilu eksperckim utrzymuje
ograniczenie roli. Istniejące sygnały kodu/kontekstu i klasyfikacja trudności są
używane bez zmiany ich progów. Nie jest to kompletna klasyfikacja każdej możliwej
prośby w języku naturalnym.

Rola `research` przechodzi do Conversation, zamiast automatycznego Planning.
Proste wyszukiwanie może użyć mniejszego modelu z narzędziami; jawne Hard lub sygnały
kodu/trudnej analizy nadal uruchamiają bramkę. Modele pomocnicze do streszczeń,
osadzeń i szybkiej mowy zachowują swoje zadania. Gdy brak dopuszczonego modelu
lokalnego i dozwolonej chmury — czytelny brak trasy, bez wywołania słabego lokalnego.

Bramki prywatności, capabilities, zdrowia, limitów i budżetu zachowano. Dobór
kont/chmury i istniejąca hierarchia nie zostały zastąpione nowym katalogiem modeli.
Nie nadano modelom dostępu do terminala lub Brokera przez zmianę roli modelu.

## Zapisane RED i błędy

- Pierwotne 6 regresji: **0/6, exit101**; po poprawce **6/0**.
- Dodatkowe obejście wskazówką i mapowanie research: **6/2, exit101**.
- Krótki follow-up po ekspercie: **0/1, exit101**. Po zmianie pokryty końcowym zestawem.
- Pierwsza kompilacja szerszego modułu: linker SIGBUS, dysk 100%, ok. 6,3 MiB wolnego.
  To nie behawioralny RED. Usunięto wyłącznie własny, odtwarzalny cache 14B po
  potwierdzeniu braku mapowania przez żywe procesy. Zwolniono 7321313664 B; cache 8B,
  raporty, logi i publiczne piny zachowano. Nie deklarujemy ponownego hashowania wag.
- Następny przebieg wykazał 8 starych oczekiwań dopuszczających słaby lokalny fallback
  lub nieznany profil 27B. Zmieniono testowy profil na obecnie wybrany, ze znanymi
  capabilities, i usunięto słabe trasy z oczekiwanych wyników Kod/Hard/review.
  Test jurysdykcji korzysta z Conversation, by nadal izolować jurysdykcję od roli.
  Nie zmieniono zamrożonych evals ani progów.
- Jeden nieużywany uchwyt w starym fixture usunięty; końcowy Clippy bez ostrzeżeń.

## Kontrole końcowe

| Kontrola | Wynik |
|---|---|
| router-contract + router-impl + router-fake, wszystkie testy | **80/0/0**, w tym 13 nowych kontroli ról |
| Clippy tych 3 pakietów, all-targets, -D warnings | **0** |
| app-modules --lib | **50/0/1 ignored** |
| app-core: routing, model_routing, pipeline_routing | **7/0/0** |
| cargo fmt --all -- --check | **0** |
| katalog i HTML --check/--verify | **0,105 stron /0 problemów** |

Łącznie **137 zaliczonych /0 błędów /1 pominięty**. Istniejący pominięty test
AzureOpenAI czeka na decyzję właściciela o rejestrze zgodności; nie zmieniano polityki
ani nie traktowano pominięcia jako zaliczenia. Nie powtarzano całego workspace/UI.

[Surowe dowody i manifest SHA256](artifacts/2026-11-12-router-expert-roles/README.md)
zachowują komendy, źródła, wszystkie porażki i końcowe wyniki. Nie przypisujemy
lokalnych wyników nowemu CI, dopóki ten commit nie ma własnego zakończonego przebiegu.

Poprzednie CI199 na a836bb57 zakończone success przed publikacją. Zapisano jego
stan, bez przenoszenia tego wyniku na nowy kod. Nowa gałąź dostanie własne CI;
żadnego starszego przebiegu nie anulowano lub ponowiono.

## Granice i dalsze kroki

To testy deterministycznego routera w Linux i z kontrolowanymi dostawcami;
nie realna inferencja 27B, prawdziwe API, przeglądarka, Windows/WebView2, mikrofon lub GPU.
Test research dowodzi wyboru trasy i zachowania schematu narzędzi, nie wykonania
wyszukiwania, jakości wyników lub poprawności odpowiedzi modelu. Pozostałe testy
hybrydy zachowują identyfikatory wyników narzędzi i brak ponowienia po rozpoczęciu
narzędzia na portach testowych.

27B Q4 nadal nie mieści się w całości w RTX 2060 12 GB. Nie przełączono katalogu,
nie pobrano 27B, nie zmieniono domyślnego modelu rozmowy. Wybór offload/RAM lub
większej karty pozostaje otwarty. Bez 27B lokalny Kod ma teraz wymagać dozwolonej
chmury albo zatrzymać się; to celowa zmiana wobec wcześniejszego działania na 8B.

#58/#77 i odbiór modeli nadal otwarte: bramka nie usuwa błędnego opisu historii
przez modele. Potrzebne rzeczywiste testy read/empty/metadata-only/odmowa/Undo,
autonomiczna pętla kodowania, wszystkie persony/tryby, uprawnione API oraz właściwy
sprzęt. Potrzebny też świeży niezależny przegląd i zielone CI przed scaleniem.
Nie uruchomiono subagentów, harmonogramów ani mostów CLI; nie odczytywano sekretów.

Uściślono też historyczny opis kontroli dokumentacji w archiwumCPU: Prettier pomija
`docs` zgodnie z `.prettierignore`, więc jego exit0 nie poświadcza formatowania
Markdown. Dokumentacja ma własne rzeczywiste kontrole generatora i verify.
