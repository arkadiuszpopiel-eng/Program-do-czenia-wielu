# Przekazanie wybranego projektu do modeli — 2026-10-24

Przyczyna zgłoszenia: powiązanie rozmowy z projektem i katalogiem było zachowane w stanie programu i widoczne w UI, ale początkowe żądanie Dyrygentki nie zawierało nazwy projektu ani aktywnej ścieżki. Prompt opisywał obsadę i delegowanie, bez wskazania, czego dotyczy ogólna prośba „Analiza programu i 5 zalecanych ulepszen”. Poprzedni test delegacji nie sprawdzał tej informacji w żądaniu HTTP.

## Poprawka

- Wspólny moduł `app-chat/project_context.rs` odczytuje bieżący projekt, tytuł rozmowy i faktyczny katalog z metadanych sesji.
- Kontekst trafia do systemowego żądania Alfy i do historii wykonawczej bezpośredniej oraz delegowanej agentki; wykonania zachowują go w lokalnym checkpoincie.
- Ogólna prośba o analizę programu dotyczy wybranego projektu/katalogu. Jawnie wskazany inny cel ma pierwszeństwo. Brak wyboru jest oznaczony jako brak wyboru.
- Wyłączenie dostępu do katalogu pozostaje skuteczne. Nazwa projektu nie jest dowodem zawartości plików. Analiza i zalecenia nie upoważniają do edycji.
- Nowe żądania otrzymują aktualny wybór, bez użycia nazwy dawnego projektu z historii. Zakres istniejącego wykonania pozostaje związany z jego katalogiem; jeśli różni się od aktualnego wyboru rozmowy, nie przypisujemy mu nowej nazwy projektu.
- Nazwy i ścieżki są kodowane jako dane JSON, a nie wstawiane jako dodatkowe instrukcje.

## Potwierdzenie

RED: rozszerzony test HTTP przed poprawką zakończył się błędem „request 0 is missing selected project” (`target/project-context-red.log`).

Końcowa regresja rdzenia: 14/14 (`target/project-context-final-tests.log`): kontekst projektu, profile uprawnień, delegowanie przez Router oraz odtwarzanie zadań. Dodatkowy końcowy test HTTP z pełnym odczytem README: 2/2 (`target/project-context-http-final.log`). Testy korzystają z AppCore, rzeczywistego Routera, OpenAiProvider i kodeka HTTP oraz lokalnego serwera modelu. Sprawdzają:

1. nazwę projektu i ścieżkę w faktycznych żądaniach Alfy i Delty;
2. obecność README w wybranym katalogu, a następnie faktyczny odczyt jego treści przez fs_read i marker w kolejnym żądaniu modelu;
3. model tekstowy na początku kolejki, za nim model z narzędziami;
4. zmianę projektu A → B bez starej nazwy w nowym kontekście;
5. usunięcie przypisania i wyłączenie katalogu (null, bez narzędzi);
6. kontekst projektu przy bezpośredniej rozmowie z Deltą.

Clippy `-D warnings`: czysty (`target/project-context-clippy.log`). Bramka rozmiarów: 0 naruszeń; nowy moduł i test również poniżej limitu pliku. Niezależny przegląd statyczny: bez P1 w zakresie poprawki. Windows: build `--locked --offline` zakończony sukcesem (`target/project-context-native-final.log`).

To test z lokalnym serwerem modelu i tymczasowym katalogiem projektu. Potwierdza przekazanie kontekstu oraz wykonanie odczytu. Nie jest płatnym wywołaniem gpt-6-luna ani potwierdzeniem treści odpowiedzi rzeczywistego modelu. Nie analizowano ani nie edytowano repozytorium Tłumacz AI właściciela.

## Wdrożenie

Podmieniono EXE i DLL w `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\Alfa-uruchomiona` o 2026-10-24T13:10:49.8872275+02:00. Sumy SHA-256 zgodne z buildem. Kopia sprzed podmiany: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa\target\project-context-rollback-20261024-131047`. Program pozostawiono zamknięty. Nie zmieniano przypisania projektu, katalogu, kont ani uprawnień użytkownika.

- `alfa-desktop.exe`: `85DC8C797A6493707DE908FDFA1305947969427ACD3C82E652974C9830591B73`
- `alfa_desktop_lib.dll`: `AF24C81B953719BC84FC2315243E1A7EC2B666C5334D422434C4EBBDA7424B20`
