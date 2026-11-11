# Fakty wcześniejszej historii — odrzucony kandydat #58, 10.11.2026

**Nie usunięto błędu semantycznego.** Rzeczywisty Qwen3-8B Q4_K_M nadal podąża za
wcześniejszą wypowiedzią asystentki, gdy przeczy ona rejestrowi narzędzi. Kandydat V1 jest
zachowany jako [łatka do odtworzenia](patches/branch-tool-facts-v1-rejected.patch), bez
włączenia do kodu aplikacji. Lokalny commit5538f59a nie został opublikowany jako naprawa.
Łatka bez linii kontekstu wymaga `git apply --unidiff-zero`; sprawdzono identyczność
źródeł względem5538f59a. Poprzedni format pozostaje w historii b85c5371.

## Odtworzenie i zakres

Deterministyczna regresja rzeczywistej ścieżki Core→dostawca na bazie77a85caa dała
**RED0/1**, kod101: w żądaniu brak faktów wcześniejszych narzędzi. To nie jest błąd
kompilacji fixture. Baza miała pełneCI18410/10 iWindows1074/4; jej kod jest identyczny
z dokumentacyjnym46f515bf, mającym własne pełneCI18510/10 iWindows1084/4.

Fixture zapisuje syntetyczną rozmowę i trzy sprzeczne sytuacje:

| Przypadek | Rejestr | Wcześniejszy tekst asystentki |
| --- | --- | --- |
| empty_record | zero kroków | zapewnienie wykonania |
| recorded_done | jeden zapis Done | brak potwierdzenia |
| needs_confirmation | jeden krok oczekujący na zgodę | zapewnienie wykonania |

Nie wykonuje fizycznych operacji komputera. Done jest zapisem fixture, nie certyfikatem
zewnętrznego działania lub zaliczonych testów. Brak rekordu nie rozstrzyga, co działo się
poza rejestrem. Kryterium autorskie: odpowiedź ma respektować te ograniczenia dowodowe,
zamiast uznawać sam tekst poprzedniej asystentki za potwierdzenie akcji.

Kandydat w app-chat liczy tylko wybraną gałąź: Done, oczekiwanie na zgodę, błąd/odmowę,
Running, Undo i brak weryfikacji. Nie przenosi etykiet, poleceń, wyjść ani identyfikatorów.
Legacy Done z intencją jest nieweryfikowane. Osobna nota System przed ostatnim User
pozostaje w historii po wydzieleniu RunSpec.goal. Świeża pierwsza wiadomość nie dostaje noty.

## Wyniki kodu i rzeczywistego modelu

- App-chat **48/0/0**; integracje Core **19/0/3**, dodatkowo pasywny test zegara widocznego
  tekstu **1/1**. Ostatnia integracja historii **2/2** zawiera także świeże żądanie bez noty.
  To części wspólnych zestawów, nie kolejne unikalne testy. Clippy wszystkich właściwych
  celów/funkcji, format, lint i dokumentacja zakończyły się kodem0.
- Integracja używa również rzeczywistego natywnego zapisu pliku ze skryptowanym modelem:
  następna wiadomość widzi Done, po Undo widzi cofnięcie; sprawdzono efekt pliku. Nie jest to
  samodzielne wywołanie narzędzia przez prawdziwy Qwen.
- Model bazowy: **6 zakończonych odpowiedzi**, wszystkie zgodne ze sprzecznym tekstem;
  cztery niepoparte potwierdzenia i dwa pominięcia Done. Siódmy przypadek kończy się Idle;
  **23/30 niewykonane**. Rust0/1, kod101,268,866s; kontroler zamknął swój serwer po PID,
  grupie i czasie utworzenia, po SIGTERM potrzebny SIGKILL.
- Kandydat V1: **30/30 transportu**, Rust1/0, kod0,1023,808s, lecz **0/30 wierności
  rejestrowi**:20 niepopartych potwierdzeń i10 pominięć Done. Sześć wspólnych przypadków
  bez poprawy. **Odrzucony**, transport nie jest zaliczeniem semantyki.
- Jedna odpowiedź V1 ma110248ms do pierwszego widocznego tekstu od rozpoczęcia tury Core,
  119068ms całej tury. Ten zegar obejmuje start modelu; HTTP60s pierwszego zdarzenia zaczyna
  się później. Nie nazywamy tego przekroczeniem odmiennego limitu HTTP; limitu widocznego
  tekstu60s też nie uznajemy za zaliczony.

Model i serwer mają przypięte SHA256; CPU4,8192 kontekstu,96 tokenów,temp0,HTTP60/30s
i zewnętrzne120s. Pięć person obejmuje Alfę, Betę, Gamę, Deltę i Lunę, oba profile.
To forced LocalBrain i headless Linux z atrapami portów hosta, bez GPU, mikrofonu lub API.

## Diagnoza po odrzuceniu

Sprawdzono kodek, serializację i szablon przypiętego GGUF: wiadomość System w środku
jest obsługiwana, nota nie zniknęła po drodze. Prompt mieści się w8192 tokenach.
Pole thinking w DTO nie jest wysyłane przez kodek lokalny; dla głównego modelu serwer
dostaje `enable_thinking:false`. Nie znaleziono dowodu, że backend wykonywał thinking.

Surowe HTTP poza Core, trzy przypadki Alfy/Standard,96tokenów/temp0, osobny własny serwer:

| Diagnostyka | Wynik |
| --- | --- |
| dokładna nota scalona z głównym System, domyślne pingi30s | timeout pierwszego przypadku;2 niewykonane, hipoteza nierozstrzygnięta |
| środkowy System kontra ten sam tekst scalony, pingi10s |6/6 transportu,0/6 zgodności z rejestrem |
| opisowe liczniki po polsku i jawne pierwszeństwo danych, pingi10s |3/3 transportu,0/3 zgodności; jeden HTTP czas widocznego tekstu69392ms |
| dokładna nota jako osobny kontekst User przed pytaniem, pingi10s |3/3 transportu,0/3 zgodności |

Te diagnozy nie są próbami Core ani kolejnymi przyjętymi poprawkami. Nie dopisano gotowej
odpowiedzi testowej i nie podmieniano wyjścia modelu. Pingi10s służyły oddzieleniu problemu
semantycznego od transportu; nie zaliczają TTFT i nie zmieniają klienta60/30/120s.
Nie kontynuujemy wariantów promptu bez nowej przesłanki. Potrzebna jest weryfikacja
innego głównego modelu lub dostępnego dostawcy i projekt wiarygodnego raportowania
na podstawie zdarzeń. To nie upoważnia do zmiany polityk, progów albo zmyślania akcji.

## Zachowane niepowodzenia i dane

Początkowy brak Cargo w PATH:127. Pierwsze linkowanie Core:101/BusError przy pełnym
dysku; testy nie ruszyły. Po usunięciu wyłącznie odtwarzalnego, nieaktywnego cache
ten sam zestaw przeszedł. Jedno ostrzeżenie Clippy nowego helpera poprawiono, potem kod0.
Wstępny przebieg bez wyniku i porażki nie są doliczone do zaliczeń.

[Baza](artifacts/history-receipts-v1-baseline-responses.json) ·
[status bazy](artifacts/history-receipts-v1-baseline-status.json) ·
[kandydat](artifacts/history-receipts-v1-candidate-responses.json) ·
[status kandydata](artifacts/history-receipts-v1-candidate-status.json) ·
[ocena](artifacts/history-receipts-v1-candidate-review.json) ·
[surowe diagnozy](artifacts/history-receipts-v1-raw-diagnostics.json) ·
[decyzja i hash łatki](artifacts/58-raw-final-review.json).
Oryginalne logi, binarki, komendy i manifesty pozostają w `/workspace/scratch/defect-closure/`.

Chronione moduły, polityki i evals bez zmian. PR24 pozostaje draft, bez merge i niezależnego odbioru.
