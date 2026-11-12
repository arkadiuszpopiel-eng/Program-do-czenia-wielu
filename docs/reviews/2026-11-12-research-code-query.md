# Research: kod w prostej frazie nie jest zadaniem programistycznym

Kontrola po publikacji [PR89](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/89)
ujawniła przypadek „Znajdź dokumentację: fn main() {}”. Mimo jawnej roli research
router traktował sam fragment kodu jak zamiar programowania i wybierał eksperta.
Regresja na62f8e573: **0/1, exit101**, faktycznie wybrany ekspert zamiast małego modelu.

Korekta jest ograniczona do jawnej roli research. Sygnał samego kodu lub kontekstu
kodu nie wymusza w niej profilu eksperckiego; klasyfikacja trudności nadal działa.
Jawne Hard, oszacowanie Hard, eskalacja po ekspercie i typowane Code/Planning
zachowują ograniczenia poprzedniego przyrostu. Zwykła Conversation nie dostała tego
wyjątku. Nie zmieniono progów klasyfikatora, polityk bezpieczeństwa lub uprawnień.

Nowy test przechodzi przez RoutedProvider: prosta fraza wybiera mały model,
ta sama fraza z Hard wybiera eksperta, a typowana klasa Code mimo research wymaga
eksperta. Modelami są kontrolowane porty. Nie wykonano prawdziwego wyszukiwania,
inferencji27B, API, GPU, Windows/WebView2 lub mikrofonu.

## Walidacja tego przyrostu

- router-contract/impl/fake: **81/0/0**, w tym14 kontroli ról.
- Clippy3pakiety/all-targets -D warnings: **0**.
- fmt --check: **0**; katalog/HTML/verify: **0,105 stron bez problemów**.

Integracja app-modules50/0/1ignored oraz app-core7/0/0 jest udokumentowana osobno
na **62f8e573**, razem z routerem80/0/0 (łącznie137/0/1). Nie powtarzano tych
niezmienionych celów integracyjnych w tej wąskiej korekcie; nie przenosimy ich liczników
na nowy commit. Aktualny rzeczywisty przepływ wyboru roli pokrywa nowy test routera.
[Dowody poprzedniego przyrostu](2026-11-12-router-expert-roles.md).

[Surowe logi, źródła i SHA256](artifacts/2026-11-12-research-code/README.md).
CI200 dla poprzedniego heada pozostawiono; korekta jest osobną gałęzią, bez pusha
anulującego tamten przebieg. Nowy commit wymaga własnegoCI i niezależnego przeglądu.
Model27B nadal niezainstalowany/nieodebrany. #41/#48/#58/#77 pozostają otwarte.
