# Regresja delegowania Alfy — poprawka 2026-10-24

Przyczyna: poprzednia poprawka sprawdzała wyłącznie enumerowaną mapę `ProviderCapabilities.models`. Brak pozycji dla modelu został potraktowany jak `tools=false`. Adapter potrafił obsługiwać narzędzia przez efektywne domyślne możliwości, ale Dyrygentka nie otrzymywała `delegate_task`. Prompt nadal opisywał delegowanie, co dawało sprzeczny komunikat widoczny na zrzucie użytkownika. Poprzednie testy ze skryptowanym dostawcą miały kompletny wpis capabilities i nie wykryły tego przypadku.

## Zmiana

- `ModelProvider.model_capabilities(model)` zwraca efektywne możliwości konkretnego modelu. OpenAI, Anthropic i Azure używają tego samego rejestru co kodek żądania.
- Arc, alias konta, dekoratory obserwacji/diagnostyki i RoutedProvider przekazują zapytanie dalej.
- Router oraz przygotowanie czatu korzystają z tej metody. Brak wiedzy nie oznacza potwierdzonego zakazu narzędzi; jawne `tools=false` pozostaje wiążące.
- Wyszukiwana jest obsługa narzędzi również w dozwolonych fallbackach. Wymaganie delegacji pozostaje w żądaniu, więc Router pomija potwierdzony model tekstowy i wybiera następny odpowiedni model w kolejce.
- Gdy cała znana kolejka jest tekstowa, kontekst mówi o braku obsługi narzędzi w modelach zamiast braku uprawnień Brokera i nie każe zapowiadać delegacji.

## Dowody

Nowy `crates/app-core/tests/conductor_dynamic_model.rs` przechodzi przez pełny AppCore, rzeczywisty Router, OpenAiProvider, kodek HTTP i lokalny serwer. Odtwarza brak modelu w statycznej mapie. Sprawdza faktyczny payload `delegate_task`, wykonany przez Deltę `fs_list` w katalogu testowej sesji i powrót wyniku do Alfy (cztery żądania HTTP). Drugi przypadek ma model tekstowy na początku kolejki i model z narzędziami za nim. Oba zielone. Nie jest to płatne API ani skan katalogu projektu właściciela.

- RED pierwszej regresji: `target/conductor-dynamic-red.log` (brak tools w rzeczywistym payloadzie).
- RED kolejki tekstowej: `target/conductor-dynamic-route-red.log`.
- Końcowe testy rdzenia (nowe scenariusze, dotychczasowa delegacja, kolejki kont, profile i odzyskiwanie): 20/20; `target/conductor-dynamic-final-tests.log`.
- Adaptery API i Router: 93 testy, 0 porażek; `target/conductor-dynamic-providers.log`.
- Clippy `-D warnings` dla app-chat/providers-api-impl/router-impl: bez ostrzeżeń; `target/conductor-dynamic-clippy.log`.
- Bramka rozmiarów: 0 naruszeń; `target/conductor-dynamic-size.log`. Nowy test również mieści się poniżej 400 linii.
- Niezależny przegląd statyczny: bez P1 w tej poprawce, potwierdzone zachowanie jawnego `tools=false`.
- Windows: końcowy build `--locked --offline` zakończony sukcesem; `target/conductor-dynamic-native.log`.

Odczyt ustawień właściciela: kolejka rozmowy zaczyna się od gpt-6-luna, Delta ma profil full. Nie zmieniano ról, katalogu, kont, limitów ani uprawnień Brokera.

## Wdrożenie

Podmieniono EXE i DLL w `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\Alfa-uruchomiona` o 2026-10-24T12:51:46.0238320+02:00. Sumy docelowe SHA-256 zgodne z buildem. Program pozostawiono zamknięty. Kopia przed podmianą: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa\target\conductor-capabilities-rollback-20261024-125143`.

- `alfa-desktop.exe`: `B5122211D1F9CEDC07F7091BC83DFAF4E3F7197F0B0AF33C276586A17EC83470`
- `alfa_desktop_lib.dll`: `DAC29EDECB6BAF005B622D6B654F77DB5CBF611E1A4209772F536BBB3CE8CB1F`

Nie wykonano płatnego wywołania na koncie właściciela ani nowego testu jego natywnego GUI. Stary skrót „Alfa (tryb deweloperski)” na pulpicie wskazuje nieistniejące D:\alfa; nie zmieniano go. Zaktualizowany program znajduje się w powyższym katalogu uruchomieniowym.
