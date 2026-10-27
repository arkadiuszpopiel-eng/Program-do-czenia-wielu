# Regresja delegate_task — 2026-10-24

Nie wolno usuwać narzędzi Dyrygentki tylko dlatego, że model nie występuje w enumerowanej mapie capabilities. Adapter rozstrzyga efektywne możliwości tak samo jak przy budowaniu żądania; Router i czat korzystają z tej odpowiedzi. Jawne tools=false pozostaje wiążące. Gdy w dozwolonej kolejce jest kolejny model z narzędziami, żądanie ma zachować wymaganie narzędzi i Router pomija model tekstowy. Zachowane są profile dostępu, zgody i kolejność dozwolonych modeli.

Test akceptacji: pełny AppCore, rzeczywisty Router, OpenAiProvider i kodek HTTP. Model spoza mapy statycznej: faktyczny payload delegate_task, Delta wykonuje fs_list, odpowiedź wraca do Alfy. Drugi przypadek: model tekstowy przed nim w kolejce. Negatywne przypadki potwierdza regresja kolejek custom endpoint bez tools i profili off/read.
