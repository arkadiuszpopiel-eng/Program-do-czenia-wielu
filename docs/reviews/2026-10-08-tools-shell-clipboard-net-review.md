# Przegląd narzędzi powłoki, schowka i sieci — 2026-10-08

Przegląd w osobnej sesji ze świeżym kontekstem (ADR 0016), **tylko odczyt kodu — nic nie uruchomiono i nic nie
zmieniono**. Zakres: `tools-shell-contract`, `tools-shell-impl`, `tools-clipboard-contract`, `tools-clipboard-impl`,
`tools-net-contract`, `tools-net-impl`. Ustalenia nie zostały jeszcze zweryfikowane testem ani wdrożone.

Zastrzeżenie do ustaleń powłoki: dotyczą faktów i deny-list samego analizatora narzędzia. Nie sprawdzono, czy twarde
reguły Brokera (`check_command`) łapią te same polecenia niezależnie. Analizator poleceń jest polityką bezpieczeństwa —
zmiany wymagają przeglądu właściciela (AGENTS.md).

## Powłoka

| Nr   | Ustalenie                                                                                                                                                                                                                                                                                           | Miejsce                                                   | Pewność                |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | ---------------------- |
| S-01 | Tekst w cudzysłowie albo po znaku ucieczki ukrywa przed analizatorem polecenia niszczące i sieciowe: `cmd /c "del /s /q ."`, `pwsh -c "Remove-Item …"`, `iex "…"`, `` `" `` w PowerShell, `^"` i apostrof w cmd, `"$(Remove-Item …)"`. Wynik: brak flagi usuwania, brak ścieżek, brak prośby o sieć | `tools-shell-contract/src/analysis.rs` (`tokens`, `verb`) | potwierdzone odczytem  |
| S-02 | Luki w wykrywaniu sieci: `Net.Sockets`, `Net.Dns`, `Net.Http` bez `System.`; brak `python`, `node`, `npm`, `pip`, `cargo`; `pip install` i `winget install` nie proszą o wyjście do sieci                                                                                                           | `analysis.rs` (lista znaczników sieci)                    | potwierdzone odczytem  |
| S-03 | Ścieżka, której nie da się rozwiązać (wieloznacznik w środku, nieznane `%VAR%`, `..` ponad korzeń), jest pomijana dla poleceń innych niż usuwanie — bez deny-listy i bez zdolności poza zakresem. Przykład: `Get-Content C:\Users\ala\.s?h\id_rsa`                                                  | `tools-shell-impl/src/run.rs`                             | potwierdzone odczytem  |
| S-04 | Za wąska lista poleceń „usuwających”: brak `robocopy /MIR`, `git clean -fdx`, `git reset --hard`, `[IO.File]::Delete`, `format`, nadpisań przez `>` i `Set-Content`                                                                                                                                 | `analysis.rs`                                             | potwierdzone odczytem  |
| S-05 | Wyjście procesu dekodowane tylko jako UTF-8 — polskie znaki z cmd/PowerShell (strona kodowa OEM) mogą trafiać do modelu jako znaki zastępcze                                                                                                                                                        | `tools-shell-impl/src/run.rs`                             | podejrzenie (ok. 70 %) |
| S-06 | Fałszywe odmowy: `pause`, `read-host`, `runas` jako argument (np. `git log --grep pause`) dają „interaktywne”; `curl -o out.html …` prosi o zgodę na „host” `out.html`                                                                                                                              | `analysis.rs`                                             | potwierdzone odczytem  |
| S-07 | Panika w ścieżce produkcyjnej: `std::env::vars()` przy zmiennej środowiskowej niebędącej poprawnym Unicode                                                                                                                                                                                          | `tools-shell-impl/src/lib.rs` (`ShellTools::new`)         | potwierdzone odczytem  |

Sprawdzone i bez uwag: limit czasu i anulowanie (zabijanie drzewa procesów), limity wyjścia, filtrowanie środowiska,
zwalnianie autoryzacji przy każdym wcześniejszym wyjściu, cytowanie argumentów cmd.

## Schowek

| Nr   | Ustalenie                                                                                                                                                                                                      | Miejsce                           | Pewność               |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- | --------------------- |
| C-01 | „Cofnij” usuwa wpis z rejestru przed przywróceniem schowka — gdy przywrócenie się nie uda (zajęty schowek), pierwsza próba daje błąd, druga „nieznany wpis”, a poprzednia zawartość przepada                   | `tools-clipboard-impl/src/lib.rs` | potwierdzone odczytem |
| C-02 | Schowek oznaczony jako poufny (menedżer haseł) blokuje każdy zapis; zawartość w nieobsługiwanym formacie jest po „Cofnij” zastępowana pustym schowkiem. Niespójne z warstwą platformy — do decyzji właściciela | `tools-clipboard-impl/src/lib.rs` | potwierdzone odczytem |
| C-03 | Rejestr cofania może trzymać dużo pamięci (do 50 wpisów po kilka–kilkanaście MB) i poprzednią zawartość schowka bez redakcji                                                                                   | `tools-clipboard-impl/src/lib.rs` | potwierdzone odczytem |

Sprawdzone i bez uwag: redakcja i przycinanie przy odczycie, limit obrazu, deny-lista plików, zgłoszenie skażenia.

## Sieć

| Nr   | Ustalenie                                                                                                                                                            | Miejsce                       | Pewność               |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | --------------------- |
| N-01 | Kodowanie odpowiedzi jest ignorowane — strony w `iso-8859-2` / `windows-1250` wracają ze znakami zastępczymi, a UTF-16 jest zgłaszane jako treść nietekstowa         | `tools-net-impl/src/fetch.rs` | potwierdzone odczytem |
| N-02 | Nagłówek `Location` jest przycinany do 2048 znaków przed obsługą przekierowania — dłuższy cel jest po cichu ucinany i pobierany jest inny adres (powinna być odmowa) | `tools-net-impl/src/http.rs`  | potwierdzone odczytem |

Sprawdzone i bez uwag: zgoda na każdy nowy host przy przekierowaniu, limit przekierowań, deny-lista domen przed Brokerem,
zwalnianie autoryzacji, limit bajtów, pobieranie z SHA-256 i usuwaniem pliku częściowego. Blokowanie localhost, zakresów
prywatnych i DNS rebinding jest w `lib-netguard` — nieprzejrzane.

Niezweryfikowane (nie liczone jako ustalenie): różna interpretacja ukośnika wstecznego w `http://evil.com\@good.com/`
przez analizator i .NET.

## Proponowany podział

- **Bez decyzji właściciela (zwykłe błędy):** S-07, C-01, N-02; N-01 po sprawdzeniu, czy `encoding_rs` jest już w
  `Cargo.lock`; S-05 po potwierdzeniu na żywo.
- **Decyzja właściciela (polityka bezpieczeństwa albo zachowanie):** S-01, S-02, S-03, S-04, S-06, C-02, C-03.
