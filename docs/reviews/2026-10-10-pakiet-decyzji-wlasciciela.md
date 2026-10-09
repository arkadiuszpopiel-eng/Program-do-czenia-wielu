# Pakiet decyzji właściciela — bezpieczeństwo i projekty ADR (2026-10-10)

**Co to jest.** Lista wszystkich otwartych decyzji z zadania A.7 roadmapy, zebrana z raportów przeglądów, `STATUS.md`
i projektów ADR 0017–0025. Bez tych decyzji nie ruszy wdrożenie i przegląd Jądra (A.8) ani wejście na `main`.

**Jak używać.** Przy każdej pozycji w tabeli dopisz **tak** (zgoda na rekomendację), **nie** (zostaje jak jest) albo
**inaczej** (wpisz literę opcji lub własne zdanie). Szczegóły każdej pozycji są w sekcjach niżej — po 1–2 minuty.

**Podsumowanie.** 44 decyzje w 7 grupach (33 z przeglądów, 3 nowe funkcje w Jądrze, 8 podpisów pod ADR). Pięć najważniejszych:
**B-03** (STOP WSZYSTKIEGO mówi „zatrzymano”, choć Broker mógł nie zadziałać — fałszywe poczucie bezpieczeństwa),
**S-01** (polecenie w cudzysłowie ukrywa kasowanie plików przed strażnikiem), **S-03** (ścieżka z gwiazdką lub
znakiem zapytania omija listę plików zakazanych, np. kluczy), **B-01** (obniżony przez Ciebie poziom samodzielności
agentek może po cichu wrócić do wyższego) i **R-05** (silniki — pliki wykonywalne z internetu — bez przypiętej sumy
kontrolnej). Kod sprawdzony odczytem 2026-10-09: te pięć jest nadal otwarte.

**Słowniczek.** *Broker* — osobny strażnik, który zatwierdza działania agentek. *Poziom L0–L4* — ile agentka może
bez pytania (L0 nic, L4 prawie wszystko). *Sesja skażona* — rozmowa, w której pojawiła się treść z zewnątrz (strona,
plik) mogąca zawierać podstawione polecenia. *Lista zakazów* — pliki i miejsca, których agentki nie mogą dotykać.
*Jądro* — Broker, watchdog, audyt, aktualizator; każda zmiana tam wymaga Twojego przeglądu (`AGENTS.md`).

## Tabela wszystkich decyzji

| ID | O co chodzi | Rekomendacja | Nakład | Blokuje |
| --- | --- | --- | --- | --- |
| S-01 | Cudzysłów ukrywa groźne polecenie | analizować też wnętrze cudzysłowu | średnia | A.8, B.7 |
| S-02, S-04 | Za krótkie listy „sieć” i „kasowanie” | rozszerzyć listy | mała | A.8, B.7 |
| S-03 | Niejasna ścieżka omija listę zakazów | odmowa albo pytanie | średnia | A.8, B.7 |
| S-06 | Fałszywe odmowy poleceń | poprawić, z testami | mała | A.8, B.7 |
| P-03 | Powłoka agentki ma pełne prawa konta | izolacja; najpierw próba | duża | A.8 |
| W3-08 | Agentka może zmieniać `PATH` | odmowa | mała | A.8 |
| C-02 | Schowek poufny / nieznany format | blokada + jasny komunikat | mała | A.8 |
| C-03 | Historia cofania schowka w pamięci | limit i czyszczenie | mała | A.8 |
| B-01 | Obniżony poziom nie wraca po starcie | ponawiać i zapisywać | średnia | A.8 |
| B-02 | Watchdog gubi Brokera po restarcie | aktualna tożsamość Brokera | średnia | A.8 |
| B-03 | STOP zgłasza sukces mimo braku Brokera | wynik „niepełne” + baner | średnia | A.8 |
| B-04 | Karta zgody „oczekuje” po decyzji | aktualizować od razu | mała | A.8 |
| B-05 | Ekran obiecuje „pełną izolację” | pokazywać stan faktyczny | mała | A.8 |
| B-07, B-08 | Nieaktualny stan; blokada wątków | poprawić oba | mała | A.8 |
| P-01 | „Panika” nie obniża każdej agentki | obniżenie kaskadowe | średnia | A.8 |
| P3-01 | Stan bezpieczeństwa tylko w pamięci | zapis trwały w Brokerze | duża | A.8 |
| P3-02 | Tryb usługi zależy od zmiennej | wykrywać niezależnie | średnia | A.8, B.8 |
| P3-03 | Skróty Alfy działają „z automatu” | tylko klawisze fizyczne | średnia | A.8 |
| T-06, P-04 | Dowiązania wyprowadzają poza zakres | liczyć po celu; odmowa | średnia | A.8 |
| P-09 | Sieć lokalna bez pytania na L4 | zawsze pytać | średnia | A.8 |
| W3-03, W3-09 | Ścieżki sieciowe jako wysyłka | potwierdzić i domknąć | średnia | A.8 |
| W3-04 | Załączony obraz nie „skaża” sesji | skażać | mała | A.8 |
| W3-07 | Kopie zapasowe dostępne dla agentek | zakaz + szyfrowanie | średnia | A.8 |
| P2-07 | Skażenie sesji jest trwałe | potwierdzić | brak (zrobione) | A.8 |
| R-05, P3-04 | Silniki bez sumy kontrolnej | przypiąć, resztę blokować | średnia | A.8; pomaga A.4 |
| R-24 | Podwójne pobranie aktualizacji | naprawić | mała | A.8 |
| P-08, P3-06 | Brak podpisu plików programu | certyfikat przed wydaniem | duża | A.8, wydanie |
| R-20 | Pamięć: cicha akceptacja, „zapamiętaj” | jawna zgoda, znacznik źródła | średnia | A.8 |
| P2-06 | Dostęp agentki do pamięci wg roli | potwierdzić wariant ścisły | brak (zrobione) | A.8 |
| R-10 | Częściowe „Cofnij” wygląda na pełne | pokazać stan częściowy | średnia | A.8 |
| R-21 | Limit dzienny mostów się zeruje | licznik na dysku | mała | A.8, C1.2 |
| P3-07 | Narzędzia mostu omijają Brokera | przez Brokera | średnia | A.8, C1.2 |
| Pakiet drobnych | 12 małych utwardzeń | zgoda na wdrożenie pakietem | średnia | A.8 |
| C2.4 | Drugi strażnik treści w Brokerze | tak, tylko jako dodatkowy | średnia | C2.4 |
| C2.13 | Sieć firmowa i własny certyfikat | odłożyć; domyślnie wyłączone | średnia | C2.13 |
| C2.16 | Dziennik operacji na kluczach | tak | mała | C2.16 |
| ADR 0017 | Kolejność platform | podpisać | brak | B.4, B.7, G |
| ADR 0018 | Wybór platformy; Broker bez Windows | podpisać po S-01…S-06 | duża | B.4, B.7, G |
| ADR 0019, 0020 | Protokół i host bez okna | podpisać | duża | D, E, F |
| ADR 0021 | Dostęp zdalny i parowanie | podpisać | duża | D, E, F |
| ADR 0022 | Zgody z telefonu | podpisać; przełącznik ostrożnie | duża | D.5, E.4 |
| ADR 0023 | Praca bez komputera | podpisać z opcją A | duża | E.6, F.6 |
| ADR 0024 | Licencja silnika mowy | wybrać po sprawdzeniu faktów | mała–duża | C1.1, wydanie |
| ADR 0025 | Kamery i twarze | podpisać z propozycjami | duża | C3.8–C3.13 |

## 1. Polecenia powłoki i schowek

Źródło grupy: [przegląd narzędzi powłoki, schowka i sieci](2026-10-08-tools-shell-clipboard-net-review.md). Analizator
poleceń to polityka bezpieczeństwa — każda zmiana przechodzi Twój przegląd. Raport zastrzega: nie sprawdzono, czy
twarde reguły samego Brokera łapią te polecenia niezależnie (**stan do potwierdzenia** przy wdrożeniu).

### S-01 — cudzysłów ukrywa groźne polecenie

- **Problem:** strażnik poleceń traktuje tekst w cudzysłowie jak jeden nieczytelny kawałek. Przykład: agentka po przeczytaniu złośliwej strony uruchamia `cmd /c "del /s /q ."` — strażnik nie widzi kasowania, nie pyta o zgodę i nie ma czego cofnąć.
- **Opcje:** (a) czytać też wnętrze cudzysłowów i poleceń zagnieżdżonych — więcej pytań o zgodę; (b) każde polecenie z zagnieżdżoną powłoką zawsze wymaga zgody — najprostsze, najwięcej pytań; (c) zostawić — luka zostaje.
- **Rekomendacja:** (a), a czego nie da się odczytać — (b); nieczytelne polecenie ma być traktowane jak groźne, nie jak niewinne.
- **Po decyzji:** zmiana w `tools-shell-contract` (analizator) z testami na każdy przykład z raportu; przegląd człowieka. Źródło: [raport, wiersz 15](2026-10-08-tools-shell-clipboard-net-review.md).

### S-02 i S-04 — za krótkie listy „to jest sieć” i „to kasuje”

- **Problem:** strażnik zna tylko część poleceń sieciowych i kasujących. Przykład: `git clean -fdx`, `robocopy /MIR` albo nadpisanie pliku przez `>` niszczą dane bez flagi „usuwa”; `python`, `node`, `pip install` łączą się z siecią bez prośby o zgodę na sieć.
- **Opcje:** (a) rozszerzyć obie listy o pozycje z raportu; (b) odwrócić zasadę: nieznany program = pytanie o sieć — bezpieczniej, dużo więcej pytań; (c) zostawić.
- **Rekomendacja:** (a) teraz; (b) rozważyć razem z P-03, bo lista nigdy nie będzie pełna.
- **Po decyzji:** `tools-shell-contract`, testy; przegląd człowieka. Źródło: [raport, wiersze 16 i 18](2026-10-08-tools-shell-clipboard-net-review.md).

### S-03 — niejasna ścieżka omija listę zakazów

- **Problem:** gdy ścieżki nie da się jednoznacznie ustalić (znak `?` lub `*` w środku, nieznana zmienna), polecenie inne niż kasowanie jest przepuszczane bez sprawdzenia listy zakazów. Przykład: `Get-Content C:\Users\ala\.s?h\id_rsa` czyta klucz SSH, choć katalog `.ssh` jest zakazany.
- **Opcje:** (a) niejasna ścieżka = odmowa; (b) niejasna ścieżka = zawsze pytanie o zgodę z ostrzeżeniem; (c) zostawić.
- **Rekomendacja:** (a) dla ścieżek pasujących wzorcem do miejsc zakazanych, (b) dla pozostałych — tracisz wygodę wzorców w poleceniach agentek.
- **Po decyzji:** `tools-shell-impl` (`run.rs`), testy; przegląd człowieka. Źródło: [raport, wiersz 17](2026-10-08-tools-shell-clipboard-net-review.md).

### S-06 — fałszywe odmowy

- **Problem:** słowa `pause`, `read-host`, `runas` użyte jako zwykły argument dają odmowę „polecenie interaktywne”, a `curl -o out.html …` prosi o zgodę na nieistniejący serwer `out.html`. Przykład: `git log --grep pause` nie działa. To nie luka, tylko uciążliwość — ale uczy klikać „zgoda” bez czytania.
- **Opcje:** (a) sprawdzać te słowa tylko w miejscu nazwy polecenia i rozpoznawać nazwę pliku po `-o`; (b) zostawić.
- **Rekomendacja:** (a), z testem, że prawdziwe `runas` i `pause` nadal są odrzucane — żadne zabezpieczenie nie słabnie.
- **Po decyzji:** `tools-shell-contract`, testy; przegląd człowieka. Źródło: [raport, wiersz 20](2026-10-08-tools-shell-clipboard-net-review.md).

### P-03 — powłoka agentki działa z pełnymi prawami Twojego konta

- **Problem:** polecenia agentek uruchamiają się z takimi prawami jak Ty; jedyną barierą jest strażnik czytający tekst polecenia, a ten da się oszukać (S-01…S-04, sklejanie nazw z kawałków). Przykład: sprytnie zapisane polecenie zamyka inny program albo czyta pliki, których strażnik nie skojarzył.
- **Opcje:** (a) proces o obniżonych prawach, zapis tylko przez Brokera; (b) pełna „piaskownica” Windows (AppContainer) — najmocniej, część narzędzi przestanie działać; (c) zostawić i przyjąć, że strażnik tekstowy to jedyna bariera.
- **Rekomendacja:** (a), poprzedzone krótką próbą na sprzęcie — **nie wiem**, ile narzędzi programistycznych zniesie obniżone prawa.
- **Po decyzji:** warstwa platformy i `tools-shell-impl`; decyzja architektoniczna z modelu zagrożeń, przegląd człowieka. Źródło: [przegląd #1, wiersz 39](2026-10-security-review-1.md).

### W3-08 — agentka może zmieniać `PATH`

- **Problem:** `PATH` to lista katalogów, w których Windows szuka programów. Agentka (za zgodą przy każdym zapisie) może dopisać tam katalog. Przykład: podstawiony katalog z fałszywym `claude.cmd` przechwytuje program, który uruchomisz później sam, już poza nadzorem Brokera.
- **Opcje:** (a) odmowa zawsze; (b) tylko dopisanie na końcu, z osobną kartą pokazującą dodane katalogi; (c) zostawić.
- **Rekomendacja:** (a) — `PATH` zmieniasz sam; tracisz automatyczne „doinstaluj i dodaj do PATH” przez agentkę.
- **Po decyzji:** `platform-apps-contract` (polityka narzędzia), test; przegląd człowieka. Źródło: [przegląd fali 3, wiersz 38](2026-10-wave3-review.md).

### C-02 — schowek z hasłem i schowek w nieznanym formacie

- **Problem:** gdy w schowku jest treść oznaczona jako poufna (np. z menedżera haseł), agentka nie może nic do niego wstawić. Gdy jest tam format, którego Alfa nie obsługuje, „Cofnij” przywraca pusty schowek. Przykład: skopiowany fragment arkusza przepada po cofnięciu wstawki agentki.
- **Opcje:** (a) blokada zostaje, ale z jasnym komunikatem; przy nieznanym formacie agentka uprzedza, że cofnięcie nie odtworzy zawartości; (b) pozwolić na zapis bez odczytu poprzedniej zawartości i bez „Cofnij” — wygodniej, hasło w schowku zostaje nadpisane; (c) zostawić.
- **Rekomendacja:** (a) — Alfa nigdy nie dotyka schowka z hasłem i nie obiecuje cofnięcia, którego nie wykona.
- **Po decyzji:** `tools-clipboard-impl`, ujednolicenie z warstwą platformy. Źródło: [raport, wiersz 31](2026-10-08-tools-shell-clipboard-net-review.md).

### C-03 — historia cofania schowka zajmuje pamięć i trzyma dane

- **Problem:** Alfa pamięta do 50 poprzednich zawartości schowka (każda nawet kilkanaście MB), bez wycinania sekretów. Przykład: po godzinie pracy z obrazami program zajmuje setki MB, a dawno skopiowany numer konta nadal siedzi w pamięci.
- **Opcje:** (a) limit łącznego rozmiaru oraz czyszczenie przy końcu sesji i po STOP; (b) dodatkowo nie zapamiętywać treści wyglądającej na sekret — wtedy tego kroku nie da się cofnąć; (c) zostawić.
- **Rekomendacja:** (a) i (b). Wartość limitu ustalasz Ty; raport jej nie podaje.
- **Po decyzji:** `tools-clipboard-impl`, testy. Źródło: [raport, wiersz 32](2026-10-08-tools-shell-clipboard-net-review.md).

## 2. Użycie Brokera i Jądro

Źródło B-…: [przegląd użycia Brokera](2026-10-08-broker-usage-review.md). Cała grupa to obszar Jądra bezpieczeństwa
— zmiany przygotuje sesja, zatwierdzasz Ty. B-06 jest już poprawione w interfejsie i nie wymaga decyzji.

### B-01 — obniżony poziom samodzielności nie wraca po starcie

- **Problem:** jeśli Broker wstaje wolniej niż aplikacja, zapisane obniżenie poziomu nie jest przywracane i nikt tego nie ponawia; obniżenia dla pojedynczej rozmowy nie są przywracane nigdy. Przykład: ustawiasz L1, restart, antywirus spowalnia start — Broker pracuje na domyślnym L3.
- **Opcje:** (a) ponawiać po połączeniu, brak łącza traktować jako „nieznany”, zapisywać też obniżenia dla rozmów; (b) tylko ponawianie; (c) zostawić.
- **Rekomendacja:** (a) — poziom ustawiony przez Ciebie ma obowiązywać zawsze.
- **Po decyzji:** `app-core` (`permissions.rs`), `app-broker`; przegląd człowieka. Powiązane z P3-01. Źródło: [raport, wiersz 18](2026-10-08-broker-usage-review.md).

### B-02 — watchdog gubi Brokera po jego restarcie

- **Problem:** watchdog (niezależny „wyłącznik awaryjny” pod `Ctrl+Shift+F12`) pamięta numer procesu Brokera tylko z chwili startu. Przykład: Broker pada i wstaje na nowo; skrót zabija procesy lokalne, ale nowy Broker nie cofa zgód — zostaje droga przez aplikację, która mogła właśnie się zawiesić.
- **Opcje:** (a) przekazywać watchdogowi aktualny numer procesu; (b) rozpoznawać Brokera po pliku programu i koncie zamiast po numerze; (c) zostawić.
- **Rekomendacja:** naprawić; wybór (a)/(b) zostawić przeglądowi — **nie rozstrzygam**, który wariant jest odporniejszy na podszycie.
- **Po decyzji:** `app-broker`, `app-safety` (watchdog); przegląd człowieka. Źródło: [raport, wiersz 19](2026-10-08-broker-usage-review.md).

### B-03 — STOP WSZYSTKIEGO zgłasza sukces, gdy Broker nie zadziałał

- **Problem:** aplikacja zawsze pokazuje „zatrzymano pracę agentek”, nawet gdy Broker nie odpowiedział w 500 ms. Przykład: Broker wisi, zgody nie zostały cofnięte, wpisu w audycie nie ma — a Ty jesteś przekonany, że wszystko stoi.
- **Opcje:** (a) osobny wynik „zatrzymanie niepełne: Broker nieosiągnięty” z trwałym banerem; (b) dodatkowo osobne połączenie pierwszeństwa dla STOP; (c) zostawić.
- **Rekomendacja:** (a) od razu, (b) do oceny w przeglądzie — przycisk awaryjny nie może kłamać.
- **Po decyzji:** `app-modules` (`broker.rs`), `app-core`, komunikat w interfejsie; przegląd człowieka. Źródło: [raport, wiersz 20](2026-10-08-broker-usage-review.md).

### B-04 — karta zgody dalej „oczekuje” po Twojej decyzji

- **Problem:** po zatwierdzeniu w oknie Brokera karta w rozmowie nadal pokazuje „oczekuje” z przyciskiem; po restarcie Brokera numery próśb zaczynają się od zera. Przykład: stary numer „3” wskazuje zupełnie inną, nową prośbę.
- **Opcje:** (a) aktualizować kartę od razu i nadać prośbom numery niepowtarzalne; (b) tylko odświeżanie karty; (c) zostawić.
- **Rekomendacja:** (a) — zgoda nie może trafić do innej prośby niż ta, którą widzisz.
- **Po decyzji:** `agent-runtime-impl`, interfejs; numeracja w `safety-broker-impl` (Jądro) — przegląd człowieka. Źródło: [raport, wiersz 21](2026-10-08-broker-usage-review.md).

### B-05 — ekran obiecuje „pełną izolację”, gdy jej nie ma

- **Problem:** napis „izolacja pełna” zależy tylko od trybu pracy, nie od tego, czy połączenie z Brokerem działa. Przykład: łącze zerwane, ikona czerwona, a lista faktów obok mówi „izolacja pełna”.
- **Opcje:** (a) „pełna” tylko gdy tryb usługi i łącze działa; opis zależny od tego, czy okno zgód naprawdę działa; (b) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `app-broker` (`status.rs`), `BrokerStatusCard.svelte`; przegląd człowieka. Źródło: [raport, wiersz 22](2026-10-08-broker-usage-review.md).

### B-07 i B-08 — nieaktualny stan Brokera i zamrożony interfejs (podejrzenia)

- **Problem:** stan Brokera wczytany przy starcie może przykryć nowszą wiadomość „zerwano”, a brak danych jest pokazywany jak „bezpiecznie”; zawieszony Broker może zamrozić interfejs na kilka sekund. Przykład: brak banera ostrzegawczego mimo zerwanego łącza. **Stan do potwierdzenia** — raport oznacza oba jako podejrzenia.
- **Opcje:** (a) najpierw test potwierdzający, potem poprawka; (b) zostawić.
- **Rekomendacja:** (a) — „nie wiem” ma być pokazywane jako ostrzeżenie, nie jako spokój.
- **Po decyzji:** B-07 tylko interfejs (`broker.svelte.ts`), B-08 `app-broker`; przegląd człowieka dla B-08. Źródło: [raport, wiersze 24–25](2026-10-08-broker-usage-review.md).

### P-01 — „panika” nie obniża każdej agentki

- **Problem:** ustawienie dla konkretnej agentki ma pierwszeństwo przed późniejszym obniżeniem ogólnym. Przykład: agentka ma osobno L4; obniżasz wszystko do L0, a ona dalej działa na L4.
- **Opcje:** (a) obniżenie ogólne obcina wszystkie ustawienia szczegółowe; (b) osobne „sufity” dla agentek; (c) zostawić.
- **Rekomendacja:** (a) — „wszystko na zero” ma znaczyć wszystko; tracisz zachowanie wyjątków po obniżeniu.
- **Po decyzji:** `safety-broker` (zmiana znaczenia ADR 0015) — Jądro, przegląd człowieka. Źródło: [przegląd #1, wiersz 37](2026-10-security-review-1.md).

### P3-01 — stan bezpieczeństwa Brokera żyje tylko w pamięci

- **Problem:** skażenie sesji, poziomy, „zawsze zezwalaj” znikają przy restarcie Brokera; obecna łatka odtwarza je z aplikacji, ale ginie razem z nią. Przykład: restart programu i rozmowa, która czytała podejrzaną stronę, znów może wysyłać dane bez pytania.
- **Opcje:** (a) Broker zapisuje stan we własnym chronionym katalogu i odtwarza przy starcie; (b) zostać przy łatce po stronie aplikacji; (c) po każdym restarcie zaczynać od najostrzejszych ustawień — bezpieczne, uciążliwe.
- **Rekomendacja:** (a); do czasu wdrożenia (c) jako stan przejściowy.
- **Po decyzji:** `safety-broker-impl` — Jądro, duży pakiet do przeglądu. Źródło: [przegląd #3, wiersz 35](2026-10-security-review-3.md).

### P3-02 — tryb usługi da się wyłączyć zmienną użytkownika

- **Problem:** aplikacja szuka konfiguracji usługi Brokera w katalogu wskazanym przez zmienną środowiskową, którą zmieni każdy program bez praw administratora. Przykład: po podmianie zmiennej Alfa po cichu przechodzi w słabszy tryb „przenośny”.
- **Opcje:** (a) pytać system o właściwy katalog albo o zarejestrowaną usługę; jeśli usługa istnieje, a nie działa — stan bezpieczny, nie tryb słabszy; (b) zostawić baner „przenośny”.
- **Rekomendacja:** (a).
- **Po decyzji:** `platform-windows-kernel-impl`, `app-broker` — Jądro; test czeka z oznaczeniem „do decyzji”. Źródło: [przegląd #3, wiersz 36](2026-10-security-review-3.md).

### P3-03 — skróty Alfy reagują na klawisze „wciśnięte” przez program

- **Problem:** `Ctrl+Alt+D` (dyktowanie), `Ctrl+Alt+R`, `Ctrl+Alt+Space` działają także, gdy naciśnie je inny program. Blokada jest tylko w narzędziu klawiatury agentek, nie w powłoce. Przykład: polecenie powłoki włącza dyktowanie i mikrofon zapisuje rozmowę w pokoju.
- **Opcje:** (a) skróty przez własny mechanizm z filtrem „tylko fizyczna klawiatura” (już istnieje w kodzie); (b) zostawić.
- **Rekomendacja:** (a); skutek uboczny: skróty nie zadziałają z programów do makr i pilotów.
- **Po decyzji:** powłoka Tauri (`shortcuts.rs`), warstwa platformy. Źródło: [przegląd #3, wiersz 37](2026-10-security-review-3.md).

## 3. Pliki, sieć i treść niezaufana

### T-06 i P-04 — dowiązania wyprowadzają poza dozwolony zakres

- **Problem:** dowiązanie to „skrót”, który wygląda jak plik w katalogu roboczym, a wskazuje gdzie indziej. Zakres zgody liczony jest z nazwy, nie z celu (T-06, podejrzenie), a dowiązanie twarde do pliku zakazanego omija listę zakazów (P-04). Przykład: plik `notatki.txt` w projekcie to w rzeczywistości plik z hasłami przeglądarki.
- **Opcje:** (a) liczyć zakres po rzeczywistym celu i odmawiać plikom z wieloma dowiązaniami w profilu; (b) zamiast odmowy pytać o zgodę; (c) zostawić. **Stan T-06 do potwierdzenia testem.**
- **Rekomendacja:** (a), z (b) tam, gdzie odmowa okaże się zbyt częsta.
- **Po decyzji:** `tools-fs-impl`, warstwa platformy, do przeglądu razem z Brokerem. Źródło: [STATUS, wiersz 178](../STATUS.md), [przegląd #1, wiersz 40](2026-10-security-review-1.md).

### P-09 — połączenia z siecią lokalną bez pytania na L4

- **Problem:** na najwyższym poziomie agentka łączy się z adresami tego komputera i sieci domowej bez pytania. Przykład: przez lokalny port przeglądarki sięga po ciasteczka logowania albo steruje routerem. Narzędzie pobierania stron ma osobną blokadę adresów lokalnych (nieprzejrzaną), więc luka dotyczy głównie reguł Brokera.
- **Opcje:** (a) połączenie lokalne zawsze wymaga zgody, na każdym poziomie; (b) twarda blokada z listą wyjątków; (c) zostawić.
- **Rekomendacja:** (a); wyjątki dla własnych usług Alfy — **do sprawdzenia**, czy są potrzebne.
- **Po decyzji:** `safety-broker`, `risk-classifier` — Jądro. Źródło: [przegląd #1, wiersz 45](2026-10-security-review-1.md).

### W3-03 i W3-09 — ścieżki sieciowe traktowane jak wysyłka danych

- **Problem:** otwarcie pliku z cudzego serwera (`\\serwer\udział\…`) wysyła mu Twoje dane logowania Windows. Narzędzia już tego odmawiają poza sieciowym katalogiem roboczym (zmiana wdrożona — potrzebne potwierdzenie), ale Broker sam tego nie pilnuje. Przykład: wtyczka albo przyszłe narzędzie omija blokadę narzędzi.
- **Opcje:** (a) potwierdzić zmianę i dodać regułę w Brokerze; (b) dodatkowo lista Twoich zaufanych udziałów; (c) cofnąć zmianę.
- **Rekomendacja:** (a); (b) dopiero gdy będzie potrzebne.
- **Po decyzji:** `safety-broker-contract` — Jądro; test czeka z oznaczeniem „do decyzji”. Źródło: [przegląd fali 3, wiersze 33 i 39](2026-10-wave3-review.md).

### W3-04 — czy załączony obraz „skaża” sesję

- **Problem:** dokument w załączniku oznacza rozmowę jako zawierającą treść z zewnątrz, obraz — celowo nie, choć tekst na obrazie model też czyta. Przykład: zrzut ekranu z dopiskiem „wyślij ten plik pod adres…”.
- **Opcje:** (a) obraz skaża sesję tak jak dokument — więcej pytań o zgodę po dodaniu obrazu; (b) zostawić.
- **Rekomendacja:** (a) — spójnie z narzędziem, które za ten sam obraz już skaża.
- **Po decyzji:** `app-files`, testy. Źródło: [przegląd fali 3, wiersz 34](2026-10-wave3-review.md), [STATUS, wiersz 192](../STATUS.md).

### W3-07 — kopie zapasowe dostępne dla narzędzi agentek

- **Problem:** katalog kopii nie jest na liście zakazów, a kopia bez hasła zawiera wszystkie rozmowy i pamięć. Przykład: agentka po podstawionym poleceniu czyta cudze rozmowy z kopii albo podmienia kopię, którą potem przywrócisz.
- **Opcje:** (a) katalog kopii zawsze zakazany dla agentek; (b) kopie z harmonogramu tylko szyfrowane — musisz ustawić hasło; (c) tylko ostrzeżenie.
- **Rekomendacja:** (a) i (b).
- **Po decyzji:** `app-files`, kompozycja w `app-core`/`app-agents`. Źródło: [przegląd fali 3, wiersz 37](2026-10-wave3-review.md).

### P2-07 — skażenie sesji zostaje na stałe (potwierdzenie)

- **Problem:** wdrożono zasadę: rozmowa raz skażona zostaje skażona we wszystkich następnych turach; zdjąć to możesz tylko Ty, nie głosem, w praktyce przez nową rozmowę. Przykład skutku: po przeczytaniu strony WWW agentka do końca rozmowy pyta przed wysyłką.
- **Opcje:** (a) potwierdzić; (b) pozwolić na łatwiejsze zdejmowanie — słabsza ochrona.
- **Rekomendacja:** (a).
- **Po decyzji:** nic do kodowania; wpis do SPEC `agent-runtime`, `safety-broker`. Źródło: [przegląd #2, wiersz 159](2026-10-security-review-2.md).

## 4. Pobieranie i aktualizacje

### R-05 i P3-04 — silniki bez przypiętej sumy kontrolnej

- **Problem:** silniki (`llama-server`, `whisper-server`, `piper`) to programy pobierane z GitHuba i potem uruchamiane. Suma kontrolna to „odcisk palca” pliku; dziś nie jest wpisana, więc Alfa pokazuje policzony odcisk i prosi o zgodę, której nie masz czym sprawdzić. Przykład: podmienione archiwum zostaje uruchomione na Twoim komputerze.
- **Opcje:** (a) wpisać sumy potwierdzonych wydań i odmawiać plików wykonywalnych bez sumy; (b) wpisać sumy, resztę zostawić za zgodą; (c) zostawić.
- **Rekomendacja:** (a). Sumy są według `STATUS.md` w przewodniku 11 — przed wpisaniem trzeba je porównać ze źródłem.
- **Po decyzji:** `app-models` (`data.rs`), test czeka z oznaczeniem „do decyzji”. Źródło: [STATUS, wiersz 101](../STATUS.md), [przegląd #3, wiersz 38](2026-10-security-review-3.md).

### R-24 — podwójne kliknięcie psuje pobraną aktualizację (podejrzenie)

- **Problem:** drugie wywołanie pobierania może zmienić stan „gotowa” na „nieudana”. Przykład: klikasz dwa razy i musisz pobierać od nowa. **Stan do potwierdzenia testem.**
- **Opcje:** (a) test, potem ignorowanie drugiego wywołania; (b) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `app-updates`, `updater-impl` — aktualizator wymaga przeglądu człowieka. Źródło: [STATUS, wiersz 163](../STATUS.md).

### P-08 i P3-06 — pliki programu bez podpisu wydawcy

- **Problem:** Alfa nie sprawdza podpisu cyfrowego własnych plików Jądra, a leżą one w katalogu, który każdy program na Twoim koncie może zmienić. Przykład: złośliwy program podmienia plik Brokera i sam zatwierdza działania.
- **Opcje:** (a) kupić certyfikat podpisu kodu, sprawdzać podpis, przenieść pliki Jądra do `Program Files`; (b) odłożyć do wydania publicznego i przyjąć ryzyko na własnym komputerze; (c) zostawić.
- **Rekomendacja:** (a) najpóźniej przed pierwszym wydaniem dla innych; certyfikat kupujesz Ty (bramka #10).
- **Po decyzji:** `app-safety`, `updater-impl`, instalator — Jądro. Źródło: [przegląd #1, wiersz 44](2026-10-security-review-1.md), [przegląd #3, wiersz 40](2026-10-security-review-3.md).

## 5. Prywatność, pamięć, cofanie i mosty

### R-20 — pamięć: cicha akceptacja i „zapamiętaj” (podejrzenie)

- **Problem:** poprawienie propozycji wpisu do pamięci od razu ją zatwierdza, a „zapamiętaj” zapisuje treść odpowiedzi jako zaufaną. Przykład: zdanie podsunięte przez złośliwą stronę trafia do pamięci długoterminowej jako Twoje własne. **Stan do potwierdzenia.**
- **Opcje:** (a) edycja zostawia propozycję w „oczekujących”; treść ze skażonej rozmowy zapisywana ze znacznikiem „niezaufane”; (b) tylko pierwsza część; (c) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `memory-contract`, `app-memory`, testy. Źródło: [STATUS, wiersz 133](../STATUS.md).

### P2-06 — dostęp agentki do pamięci według roli (potwierdzenie)

- **Problem:** wdrożono wariant ściślejszy: w trakcie zadania agentka sięga tylko do pamięci przysługującej bieżącej roli, a nie wszystkim jej rolom. Kontekst zwykłego czatu nadal bierze sumę ról. Przykład: „Badaczka” czytająca internet nie zajrzy do pamięci projektu.
- **Opcje:** (a) potwierdzić wariant ścisły; (b) ścisły także dla czatu; (c) wrócić do sumy ról.
- **Rekomendacja:** (a), a (b) do rozważenia — **nie wiem**, czy nie pogorszy odpowiedzi w czacie.
- **Po decyzji:** wpis do SPEC `memory`; (b) to zmiana w `app-memory`. Źródło: [przegląd #2, wiersz 161](2026-10-security-review-2.md).

### R-10 — częściowe „Cofnij” wygląda na pełne

- **Problem:** gdy cofnięcie uda się tylko dla części plików, dziennik zapisuje „wykonane”, a przycisk dalej jest widoczny, choć reszty nie da się już przywrócić. Przykład: z 10 plików wróciło 6, a Ty dowiadujesz się o tym po tygodniu.
- **Opcje:** (a) osobny stan „cofnięto częściowo” z listą plików; wygasłe kroki bez przycisku; (b) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `undo-journal-contract`, `app-core`, `UndoCard.svelte`; według `STATUS.md` dotyka adaptera Brokera — przegląd człowieka. Źródło: [STATUS, wiersz 123](../STATUS.md).

### R-21 — dzienny limit uruchomień mostu zeruje się sam

- **Problem:** licznik uruchomień mostu (połączenia z zewnętrznym programem `claude`/`codex`) z harmonogramu jest tylko w pamięci. Przykład: restart albo zmiana harmonogramu zeruje licznik i limit dzienny przestaje ograniczać koszty.
- **Opcje:** (a) licznik na dysku, zerowany tylko o północy; (b) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `app-bridges`, `agent-backends-impl` (bramka zgodności — polityka, przegląd człowieka). Źródło: [STATUS, wiersz 160](../STATUS.md).

### P3-07 — podstawowe narzędzia mostu omijają Brokera

- **Problem:** schowek i lista okien dla mostu działają bez pytania Brokera i bez oznaczania treści jako niezaufanej. Przykład: zewnętrzny model czyta schowek bez śladu w zgodach (sekrety są już wycinane — to naprawiono).
- **Opcje:** (a) te narzędzia także przez Brokera — więcej pytań przy pracy z mostem; (b) zostawić.
- **Rekomendacja:** (a).
- **Po decyzji:** `mcp-impl`. Źródło: [przegląd #3, wiersz 41](2026-10-security-review-3.md).

### Pakiet drobnych utwardzeń (jedna zgoda)

- **Problem:** dwanaście małych ustaleń o niskiej wadze: P-02, P-05, P-06, P-10, P-11, P-12, P-13, P2-08, P2-10, P3-05, W3-10, W3-11. Przykład (P-12): jedna zgoda na plan pokrywa kilka takich samych operacji administratora zamiast jednej.
- **Opcje:** (a) zgoda na wdrożenie całości wariantem ostrzejszym, każde z testem; (b) wybrać pojedyncze; (c) odłożyć.
- **Rekomendacja:** (a), z wyjątkiem P-05 — raport uznaje obecny stan za akceptowalny, zostawić.
- **Po decyzji:** różne moduły; części w Brokerze (P-02, P-06, P-11, P-12) idą do przeglądu człowieka. Źródło: [przegląd #1, wiersze 38–49](2026-10-security-review-1.md), [#2, wiersze 175–176](2026-10-security-review-2.md), [#3, wiersz 39](2026-10-security-review-3.md), [fala 3, wiersze 40–41](2026-10-wave3-review.md).

## 6. Nowe funkcje dotykające Jądra (C2.4, C2.13, C2.16)

- **Problem:** trzy zaplanowane funkcje wymagają Twojej zgody, bo zmieniają Brokera, reguły sieci albo audyt. Opis: [`04-nowe-funkcje.md`](../plan/04-nowe-funkcje.md), stan: [roadmapa](../plan/03-roadmapa.md).
- **C2.4 — drugi strażnik treści** (model Bielik-Guard wykrywający szkodliwe polecenia po polsku): **tak**, pod warunkiem że może tylko zaostrzyć decyzję Brokera, nigdy jej złagodzić.
- **C2.13 — sieć firmowa** (serwer pośredniczący i własny urząd certyfikacji): **odłożyć**; jeśli wdrażać, to domyślnie wyłączone — po włączeniu firma może czytać ruch do dostawców modeli.
- **C2.16 — dziennik operacji na kluczach** (kto i kiedy dodał, wymienił, wyłączył klucz; bez treści klucza): **tak**.
- **Po decyzji:** każda z trzech to osobne zadanie po A.8, z przeglądem człowieka.

## 7. Projekty decyzji architektonicznych (ADR 0017–0025)

Projekty przygotowała sesja; nie obowiązują do Twojego podpisu ([rejestr](../ADR/README.md)).

### ADR 0017 — kolejność platform

- **Co to znaczy:** najpierw Windows 11, potem panel w przeglądarce, Android 15+, na końcu Linux. To zapis planu, nie kodu — kolejność możesz zmienić wpisem w roadmapie.
- **Opcje i rekomendacja:** podpisać / zmienić kolejność (np. Linux zaraz po etapie B). Rekomendacja: podpisać.
- **Po decyzji:** odblokowuje B.4, B.7 i etap G. Źródło: [ADR 0017](../ADR/0017-strategia-wieloplatformowa.md).

### ADR 0018 — wybór platformy w jednym miejscu i Broker niezależny od Windows

- **Co to znaczy:** kod systemowy wybierany w jednym module; reguły Brokera (ścieżki, powłoki, tożsamość procesu) przestają zakładać Windows. Druga część zmienia Jądro.
- **Opcje i rekomendacja:** podpisać całość / tylko część 1 / odłożyć. Rekomendacja: podpisać, ale część 2 dopiero po S-01…S-04 i S-06 — sam ADR stawia ten warunek, żeby nie przenosić znanych dziur.
- **Po decyzji:** B.4, potem B.7 z przeglądem człowieka. Źródło: [ADR 0018](../ADR/0018-fabryka-platform-i-kontrakty-niezalezne-od-systemu.md).

### ADR 0019 i 0020 — wspólny „język” klientów i program bez okna

- **Co to znaczy:** okno, przeglądarka i telefon rozmawiają z Alfą tym samym protokołem, a Alfa może działać w tle bez okna. Uprawnienia klienta sprawdza zawsze strona komputera.
- **Opcje i rekomendacja:** podpisać / odłożyć cały dostęp zdalny. Rekomendacja: podpisać; progi (np. pamięć programu w tle) ustalasz po pomiarach.
- **Po decyzji:** etap D; bez podpisu nie powstaje kod. Źródło: [ADR 0019](../ADR/0019-protokol-niezalezny-od-transportu.md), [ADR 0020](../ADR/0020-host-bez-okna.md).

### ADR 0021 — dostęp zdalny: przekaźnik i parowanie urządzeń

- **Co to znaczy:** telefon łączy się z komputerem przez serwer pośredni, który widzi tylko zaszyfrowane dane; pierwsze urządzenie potwierdzasz kliknięciem przy komputerze. Serwer możesz postawić własny; koszt hostingu jest po Twojej stronie.
- **Opcje i rekomendacja:** podpisać / nie budować dostępu zdalnego. Rekomendacja: podpisać; dostęp zdalny zostaje wyłączony do sparowania pierwszego urządzenia.
- **Po decyzji:** D.2–D.4. Źródło: [ADR 0021](../ADR/0021-dostep-zdalny-przekaznik-e2e-i-parowanie.md).

### ADR 0022 — zatwierdzanie działań z telefonu

- **Co to znaczy:** zgody dla agentek możesz dawać z telefonu, każdą z odciskiem palca; według Twojej wcześniejszej decyzji — także wysokiego ryzyka. ADR proponuje przełącznik „to urządzenie może zatwierdzać wysokie ryzyko”, domyślnie włączony.
- **Opcje i rekomendacja:** (a) jak w ADR; (b) przełącznik domyślnie wyłączony — jedno kliknięcie więcej przy parowaniu, za to skradziony telefon nie zatwierdzi groźnych działań; (c) zdalnie tylko niskie i średnie ryzyko. Rekomendacja ostrożna: (b) — to odstępstwo od propozycji ADR.
- **Po decyzji:** zmiana kontraktu Brokera i okna zgód — Jądro; limit zgód i okno ważności ustalasz Ty. Źródło: [ADR 0022](../ADR/0022-zgody-zdalne-dowod-urzadzenia-sparowanego.md).

### ADR 0023 — rozmowa bez komputera

- **Co to znaczy:** gdy komputer jest wyłączony, telefon rozmawia wprost z modelem w chmurze; klucz API trafia na telefon tylko po osobnej zgodzie. Pytanie otwarte: czy klucz wolno trzymać w przeglądarce.
- **Opcje i rekomendacja:** A — w przeglądarce nie przechowywać (tryb tylko w aplikacji Android); B — przechowywać zaszyfrowany (ryzyko odczytu przez złośliwe rozszerzenie). Rekomendacja: podpisać z opcją A.
- **Po decyzji:** E.6, F.6. Źródło: [ADR 0023](../ADR/0023-tryb-bez-komputera-klucz-api-i-scalanie-rozmow.md).

### ADR 0024 — licencja silnika mowy

- **Co to znaczy:** rozwijana wersja silnika mowy Piper ma być na licencji GPL-3.0 (informacja ze źródła wtórnego, **do potwierdzenia**), co może ograniczać sprzedaż programu. To decyzja prawna i biznesowa.
- **Opcje i rekomendacja:** A — nowy Piper jako osobny program + opinia prawnika; B — stara wersja na MIT (bez poprawek); C — inny silnik (trzeba znaleźć polski głos). Sesja nie jest prawnikiem; ostrożnie: najpierw sprawdzić fakty u źródła, a do czasu opinii nie rozprowadzać wersji GPL.
- **Po decyzji:** wybór głosu w C1.1 i wydanie publiczne. Źródło: [ADR 0024](../ADR/0024-licencja-silnika-mowy.md).

### ADR 0025 — kamery i rozpoznawanie twarzy

- **Co to znaczy:** dziesięć zasad: kamery domyślnie wyłączone i zawsze widoczne, brak nagrywania, wzorce twarzy tylko za zgodą osoby i tylko na komputerze, rozpoznanie twarzy nigdy nie zatwierdza działań.
- **Opcje i rekomendacja:** podpisać z propozycjami / zmienić punkty z listy w sekcji 8. Rekomendacja: podpisać; przed wydaniem publicznym potrzebna opinia prawnika o danych biometrycznych.
- **Po decyzji:** C3.8–C3.13; nowe reguły Brokera dla kamer — Jądro. Źródło: [ADR 0025](../ADR/0025-kamery-i-biometria-twarzy.md).

## 8. Co jeszcze czeka na właściciela poza bezpieczeństwem

| Sprawa | Co trzeba zrobić | Blokuje |
| --- | --- | --- |
| Pakiet modeli (ok. 6 GB) | zgoda na pobranie; **stan do potwierdzenia** — dziennik roadmapy mówi, że silniki głosu i model lokalny już pobrałeś | A.4, A.5, A.6, C1.4, C2.2 |
| `gh auth login` i wypchnięcie zmian | zalogować się samodzielnie w terminalu | A.0, A.8 |
| Progi akceptacji | ustalić liczby w `ACCEPTANCE.md`, w tym próg F3-07 | wyjście z C, D, E, F, G |
| ADR 0024 — licencja silnika mowy | wybór A / B / C, ewentualnie prawnik | C1.1, wydanie |
| ADR 0025 — obraz z kamery do chmury | nigdy / za zgodą dla każdej kamery (propozycja) / zgoda ogólna | C3.7 |
| ADR 0025 — osoba nieznana i rozpoznawanie w tle | gość bez pamięci czy tylko zapisani; w tle czy tylko na żądanie | C3.8–C3.13 |
| ADR 0025 — miniatura klatki i wzorce w kopii | miniatura zostaje w rozmowie czy nie; wzorce w kopii lokalnej: nie (propozycja) / tak | C3.8–C3.13 |
| Modele widzące i modele twarzy | zgoda na pobranie | C3.6, C3.8 |
| Lepszy głos niż Piper „Gosia” | pobrać inny silnik lokalny czy użyć głosu w chmurze; głos w chmurze jest dziś niedostępny, a tekst wychodziłby poza komputer | C1.1 |
| Brzmienie mowy po strojeniu (0.0.14) | odsłuchać i ocenić | C1.1 |
| Klucze i logowanie do `claude` / `codex` | wykonać samodzielnie; próba z prawdziwym kluczem API | C1.2 |
| Wersje w manifestach Jądra | przejrzeć zmianę samego numeru wersji (bez kodu) | A.8 |
| Umiejętności: operacja „włącz” (R-23) | dodać czy zostawić wyłączanie w jedną stronę | — |
| Test „100 tokenów na sekundę” | liczyć tylko zmiany treści czy podnieść limit | A.0 (bramki) |
