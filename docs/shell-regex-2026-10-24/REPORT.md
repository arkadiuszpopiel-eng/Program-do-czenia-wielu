# Fałszywa prośba net.egress(obj|bin) — 2026-10-24

„Pełny dostęp” określa dostępność narzędzi agentki, a L4 nadal wymaga zgody na rzeczywistą wysyłkę po kontakcie sesji z niezaufaną treścią. Na zgłoszonym zrzucie cel obj|bin wskazuje jednak na fałszywą klasyfikację wzorca filtrowania katalogów jako hosta.

Odtworzono: lokalne `Get-ChildItem ... | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }` i wariant bez grupy. Dotychczasowy analizator usuwał kontekst cudzysłowów/operatora i rozpoznawał początek regexu jako UNC, dodając host oraz zewnętrzną ścieżkę. Narzędzie żądało wtedy net.egress i nieodwracalnego zakresu poza cwd. To wyzwalało ochronę Brokera widoczną w oknie.

## Poprawka

Lexer zachowuje zakresy surowego tekstu, bez zmiany publicznego wyniku tokens(). Wyjątek obejmuje bezpośredni cytowany literał po operatorze match/notmatch/replace w rozpoznanym wyrażeniu zmiennej na granicy instrukcji lub grupy. Przerwy nie mogą zawierać wywołania, nawiasów ani innej składni. Podwójne cudzysłowy z dolarem pozostają konserwatywne; dolar w pojedynczych cudzysłowach może być kotwicą regex.

Nie zmieniano Brokera, poziomu L4, taintu, reguł zgód ani allowlist. Inne UNC i URL nadal są analizowane. Jest to ograniczone rozpoznanie literału w istniejącej heurystyce poleceń, nie pełny parser PowerShell; nierozpoznane formy zachowują konserwatywne kontrole.

## Potwierdzenie

- RED odtworzył fałszywy host i ścieżkę z regexu (`target/shell-regex-red.log`).
- Przegląd wychwycił dwa zbyt szerokie wyjątki w pierwszej roboczej wersji: wykonanie skryptu UNC jako RHS oraz UNC jako rzeczywisty argument findstr.exe. Dodano RED regresje (`target/shell-regex-context-red.log`) i zamknięto je lexerem zachowującym kontekst. Wersja ta nie była wdrożona przed poprawieniem.
- Końcowe testy tools-shell-contract i tools-shell-impl: 40/40 (`target/shell-regex-tests.log`). Wśród nich: wzorce obj|bin z grupą i bez niej, warianty operatorów, zagnieżdżenie, prawdziwy UNC obok regexu, upload z rzeczywistym hostem, interpolacja, wywołanie skryptu UNC i argumenty findstr.exe.
- Test narzędzia z prawdziwym silnikiem Brokera pod FakeBroker: L4, sesja po niezaufanym pliku. Lokalny filtr uruchamia ExecPort i nie tworzy dodatkowej prośby; realny curl upload kończy się ApprovalTimeout i nie jest wykonywany bez zgody. ExecPort jest atrapą — nie wysyłano rzeczywistych danych do sieci.
- Clippy all-targets z -D warnings dla obu zmienionych pakietów: czysty (`target/shell-regex-clippy.log`). Bramka rozmiarów: 0 naruszeń (`target/shell-regex-size.log`).
- Ponowny niezależny przegląd statyczny: bez P1 w tym wąskim zakresie; potwierdzone zamknięcie obu kontrprzykładów.
- Windows build --locked --offline: sukces (`target/shell-regex-native.log`). Nie uruchamiano ponownie natywnego GUI właściciela i nie zmieniano jego ustawień.

## Wdrożenie

Podmieniono EXE i DLL w `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\Alfa-uruchomiona` o 2026-10-24T13:33:58.1158503+02:00. Sumy SHA-256 zgodne z buildem. Kopia sprzed podmiany: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa\target\shell-regex-rollback-20261024-133355`. Program pozostawiono zamknięty.

- `alfa-desktop.exe`: `9AF6020A833B50F0A13C20220F4E0B643E65D87F1ED84139E6E69D7F791EE48F`
- `alfa_desktop_lib.dll`: `EAEB010CE1E30F3AB177DAA9B576530E9299C0E7395B52D0A31B93E8568B2584`
