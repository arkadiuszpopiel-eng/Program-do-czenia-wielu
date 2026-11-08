# Przegląd poprawności fali 3 — model-recenzentka (2026-10)

Data: 2026-10-06. Gałąź: `ccr-af4b63c6-3fyzaj` (po `215e352`, commit fali 3). Recenzentka: drugi model w świeżym
kontekście (AGENTS.md, krok 6 — poprawność, bezpieczeństwo, zgodność ze SPEC; nie styl). Punkty odniesienia:
`docs/THREAT_MODEL.md`, `docs/PLAN.md` §7–§8, przeglądy #1–#3 (`docs/reviews/2026-10-security-review-{1,2,3}.md` —
ustaleń zamkniętych nie powtarzam), SPEC-i modułów fali 3.

## 1. Zakres i metoda

Kod dodany w `215e352`, czytany „jak napastnik” (agentka z wstrzykniętym poleceniem, złośliwa strona/plik/obraz,
proces tego samego konta): `tools-system-*`, `tools-net-*`, `lib-netguard`, `tools-vision-*`,
`platform-windows-ocr-impl`, `tools-media-*`, `lib-media`, `platform-apps-contract` (`sys`, `sys_policy`,
`downloads`), `platform-windows-sys-impl` (`sysport`, `procs_win`, `services_win`, `events_win`/`events_xml`,
`env_win`, `downloads`), `app-files` (załączniki, kopie zapasowe, eksport, artefakty w `.alfa`), zmiana
`transfer-contract` (suma `turns_sha256` po redakcji), `app-agents` (`sysnet.rs`, `media.rs`), migracja
`app-plugins` na `lib-netguard`. Szukane: obejścia Brokera i strażnika celów, TOCTOU (PID, ścieżki, dowiązania),
SSRF/rebinding, wstrzyknięcia argumentów (ffmpeg, XPath), parsery (przepełnienia, pętle, budżety), wycieki
sekretów/pikseli do zdarzeń, braki fail-closed, DoS.

Każde ustalenie „naprawione” ma test, który **przed poprawką nie przechodził** (uruchomiony na kodzie z `HEAD`
przed wprowadzeniem poprawki), a po niej przechodzi. Propozycje mają test `#[ignore]`, który dziś pada (Jądro), albo
— gdy poprawka leży w `app-core` (zamrożony na czas podziału, zadanie E) lub wymaga decyzji — opis z analizy kodu.
Kod Windows FFI przeczytany pod kątem `unsafe`, uchwytów, TOCTOU i walidacji danych z systemu; `cargo clippy
--target x86_64-pc-windows-msvc` czysty (§5). Wagi: skala CVSS-podobna 0–10 (wektor lokalny, agentka
z wstrzykniętym poleceniem albo złośliwa treść).

## 2. Ustalenia

| Id | Moduł | Waga | Opis | Reprodukcja | Status |
|---|---|---|---|---|---|
| W3-01 | tools-media-impl (`media_play`) | 5,5 średnia | **Nagłówek WAV wymusza wzmocnienie pamięci/CPU ×1000 (DoS).** Limit czasu odtwarzania liczony z `byte_rate` nagłówka (`lib-media`): `byte_rate = 0` → czas nieznany (limit pominięty), zawyżony → ~0 ms. Potem `decode_wav` + resampling do 48 kHz z częstotliwości z nagłówka (np. „1 Hz” → współczynnik przycięty do 1000) — **przed** sprawdzeniem długości klipu i na wątku wykonawcy tokio. 4–16 MiB takiego pliku (np. pobranego `net_download` na polecenie ze strony) = 8–32 GiB `f32` → przerwanie procesu Alfy przy braku pamięci (zerwany Broker trybu przenośnego, a cyklicznie — pętla awarii i wycofanie wersji przez watchdog/launcher, por. SR3-01). | `tools-media-impl/tests/review.rs::absurd_sample_rate_is_rejected_before_resampling` — przed poprawką FAILED (200 próbek „1 Hz” → klip 200 000 próbek) | naprawione (`play.rs`): częstotliwość 8–384 kHz i czas trwania z faktycznie zdekodowanych próbek **przed** resamplingiem; dekodowanie i resampling w `spawn_blocking`. Drugi test (`real_duration_is_checked_before_resampling`) — regresja limitu czasu przy zawyżonym `byte_rate` |
| W3-02 | platform-apps-contract (`sys_policy`, polityka narzędzia) | 4,5 średnia | **Deny-lista zapisu zmiennych użytkownika bez wielu zmiennych ładujących kod.** SPEC: „deny-lista nazw … wstrzykujących kod”; lista znała `NODE_OPTIONS`, `PYTHONSTARTUP`, `PERL5OPT`, `COR_*`, ale nie `JAVA_TOOL_OPTIONS`/`_JAVA_OPTIONS`/`JDK_JAVA_OPTIONS` (`-javaagent:` w każdej JVM), `OPENSSL_CONF`/`OPENSSL_MODULES`/`OPENSSL_ENGINES` (DLL w git, curl, Pythonie…), `PSExecutionPolicyPreference` (`Bypass`), `PYTHONHOME`, `PERL5LIB`/`PERLLIB`, `RUBYOPT`/`RUBYLIB`, `CARGO_*` (wrapper kompilatora, `runner` — właściciel buduje Alfę na tej maszynie), `NPM_CONFIG_*` (powłoka skryptów). Zapis `HKCU\Environment` działa na każdy proces uruchomiony później — trwałość poza zasięgiem Brokera. Łagodzi: `system.admin` = zgoda przy każdej eskalacji (`AdminConsent`), ale karta `setx JAVA_TOOL_OPTIONS "-javaagent:…"` nie mówi właścicielowi, że to wstrzyknięcie kodu. | `platform-apps-contract/tests/review.rs::code_loading_variables_are_denied` — przed poprawką FAILED (`JAVA_TOOL_OPTIONS`) | naprawione (`sys_policy.rs`; listy jako wycinki). **Polityka — do przeglądu człowieka**. `PATH` — W3-08 |
| W3-03 | tools-common-contract (`paths`) — dotyczy `tools-media`, `tools-vision` (fala 3) i `tools-fs`, `tools-shell`, `tools-office` | 6,5 średnia | **Ścieżka sieciowa od modelu łączy się z cudzym serwerem przed Brokerem (SMB/WebDAV + NTLM).** `resolve_path` przyjmował UNC (`\\napastnik\udział\x.wav`, WebDAV `\\host@SSL\DavWWWRoot\x`), a sprawdzenie deny-listy „także po dowiązaniach” (`protected_with_links` → `symlink_metadata`/`canonicalize`) wykonuje się **przed** prośbą do Brokera: Windows otwiera połączenie i uwierzytelnia konto właściciela (skrót Net-NTLMv2 do łamania/relay) — wysyłka bez `net.egress`, także na L4 i w sesji skażonej (THREAT_MODEL §5 „zapis do udziału” = kanał wyjścia). Po zgodzie (na L4 odczyt poza zakresem bez pytania) — dalszy odczyt nagłówków/obrazu z udziału. | `tools-media-impl/tests/review.rs::network_paths_from_the_model_are_refused_before_any_access` — przed poprawką FAILED (Broker zapytany, odczyt pliku podjęty: „nie znaleziono: \\napastnik.example\udzial\x.wav”); jednostkowe `tools-common-contract/src/netpath.rs` | naprawione: `PathError::Network` dla ścieżek sieciowych spoza **sieciowego katalogu roboczego** sesji (wybór właściciela — NAS jako katalog roboczy działa dalej); nowy moduł `netpath`. Test Q-8 (`resolves_parent_segments_lexically`) przepięty na katalog roboczy na udziale (zmiana zamierzona). Obrona w głąb po stronie Brokera — W3-09 |
| W3-04 | app-files + app-core (`turns_send`) | 5,0 średnia | **Skażenie z załączników nie dociera do Brokera (fail-open).** `FilesApp::prepare` oznacza sesję `sessions.mark_tainted` dla załączników tekstowych/dokumentów, ale flaga `SessionMeta.tainted` nie jest nigdzie zgłaszana Brokerowi (`report_untrusted_input` wołają tylko narzędzia; `grep` po `app-*`) — Broker decyduje na `SessionSecurity` w pamięci. Dokument z wstrzykniętym poleceniem dołączony przez właściciela → tura `CommandOrigin::UserText`, sesja dla Brokera czysta → `TaintedEgress` (obowiązuje także na L4) nie działa; na L3 wysyłka na hosty z allowlisty bez pytania. Obrazy celowo nie skażają (test `image_only_attachments_do_not_taint…`), choć THREAT_MODEL §10 wymienia „obraz/ekran (OCR)” jako klasę wstrzyknięć, a `tools-vision` skaża za ten sam obraz. | analiza (`app-files/src/lib.rs::prepare`, `app-core/src/commands/turns.rs::turns_send`; brak konsumenta `meta.tainted` poza `transfer`/`app-tasks`) | **naprawione (fala 4, zadanie E):** `app-chat::ChatEngine::sync_taint` — `meta.tainted` → `report_untrusted_input(…, TaintSource::File)` przed każdą generacją (`start_generation`: wysłanie z załącznikami, ponów, kontynuuj, głos, kolejka offline; błąd Brokera = tura nie startuje, czytelny błąd), przy otwarciu sesji (`app_set_active_session`) i przed przebiegiem zadania (`TaskHost::sync_taint`); restart nie zdejmuje skażenia. Test: `app-core/tests/files.rs::attachment_taint_reaches_broker_and_survives_restart` (wysyłka → `TaintedEgress`; warianty po restarcie: otwarcie sesji, tura) — przed poprawką FAILED. Obrazy: nadal decyzja człowieka (§7) |
| W3-05 | platform-apps-contract (`check_env_value`) | 2,5 niska | **Wartość zmiennej ze znakami sterującymi rozbija kartę Brokera.** `env_command` buduje `setx NAZWA "wartość"` dla karty i audytu; wartość mogła zawierać `\n`, `\r`, ESC — pierwsza linia wygląda niewinnie, reszta poza widokiem właściciela. Zmienne środowiskowe nie potrzebują znaków sterujących. | `platform-apps-contract/tests/review.rs::env_values_with_control_characters_are_refused` — przed poprawką FAILED | naprawione: odmowa znaków sterujących poza tabulatorem (`check_env_value`, ta sama funkcja w kontrakcie narzędzia i porcie) |
| W3-06 | tools-vision-impl (`vision_ocr`) | 0 — funkcjonalne (SPEC) | **Współrzędne OCR nie trafiały do modelu.** Model widzi wyłącznie `ToolOutcome::text` (`agent-runtime-impl::prompt::tool_result`); współrzędne były tylko w `data.lines`, a tekst mówił „Współrzędne linii w `lines`…” — agentka nie mogła kliknąć znalezionego tekstu (SPEC: „linie ze współrzędnymi ekranu — do kliknięcia przez `tools-input`”). | `tools-vision-impl/tests/review.rs` — przed poprawką FAILED | naprawione (`ocr.rs`): każda linia jako `[x= y= w= h=] tekst` (zredagowany, obcięty limitem); `data` bez zmian |
| W3-07 | app-files (`backup`) / kompozycja deny-list | 4,5 średnia | **Kopie zapasowe w katalogu dostępnym dla narzędzi agentek.** Katalog kopii wybiera właściciel (zwykle w profilu), a deny-listy narzędzi (`app-modules::workdir::tool_deny_lists`) go nie obejmują. Kopia bez hasła zawiera **wszystkie** sesje (poza prywatnymi), wszystkie zakresy pamięci i konfigurację: agentka z wstrzykniętym poleceniem czyta cudze sesje i pamięć (obejście izolacji sesji/zakresów — „szukanie między sesjami tylko dla właściciela”) albo **podmienia** kopię (sumy w manifeście bez uwierzytelnienia) → zatrucie pamięci/konfiguracji przy przywróceniu (S19). | analiza (`backup/mod.rs::choose_dir` sprawdza tylko, że katalog nie leży w danych Alfy; `workdir.rs::tool_deny_lists`) | propozycja: katalog kopii dopisywany dynamicznie do deny-list narzędzi (wspólny `DenyChecker` odświeżany po `choose_dir`) i/lub kopie z harmonogramu tylko szyfrowane (AEAD = także integralność); ostrzeżenie w UI dla kopii bez hasła. Wymaga zmian w kompozycji (`app-core`/`app-agents`) — decyzja człowieka |
| W3-08 | platform-apps-contract (`sys_policy`) | 3,5 niska | **`PATH` użytkownika zapisywalny przez agentkę** (świadomie — test `tests_sys::env_secrets_hidden_and_writes_denied`). Katalog dopisany przed katalogami narzędzi instalowanych per użytkownik (np. `%APPDATA%\npm` z `claude.cmd`) przechwytuje ich uruchomienie — także mostów CLI uruchamianych później przez właściciela, poza nadzorem Brokera. Łagodzi: zgoda `system.admin` przy każdym zapisie; `PATH` użytkownika jest za systemowym. | analiza | decyzja człowieka: osobna karta z listą dodanych/usuniętych katalogów, tylko dopisywanie na końcu istniejących katalogów spoza obszarów zapisywalnych przez agentki, albo odmowa |
| W3-09 | safety-broker-contract (Jądro) | 4,0 średnia | **Broker nie traktuje `fs.*` na udziale sieciowym jak wysyłki.** `fs.read(\\napastnik\udział\x)` = zwykły odczyt poza zakresem: na L4 bez pytania, także z sesji skażonej. W3-03 zamyka to w narzędziach; Broker powinien wymuszać niezależnie (obrona w głąb, także dla wtyczek i przyszłych narzędzi). | `safety-broker-contract/tests/review.rs::file_access_on_a_network_share_is_egress` (`#[ignore]`, pada: `is_egress() == false`) | propozycja: w `derive_facts` ścieżki UNC/WebDAV → `egress(serwer)` (reguły `TaintedEgress`, `EgressNotAllowlisted`), udziały z listy dozwolonej właściciela. **Ścieżka Jądra — decyzja i przegląd człowieka** |
| W3-10 | platform-windows-sys-impl (`env_win`) | 2,0 niska | **Cofnięcie zapisu zmiennej gubi typ wartości.** `user_value` zwraca tylko `REG_SZ`/`REG_EXPAND_SZ` (inne typy → `None`), a zapis wybiera typ po obecności `%`: nadpisanie istniejącej wartości innego typu (`REG_MULTI_SZ`, `REG_DWORD`) i „Cofnij” ją **usuwa**; `REG_SZ` z dosłownym `%` wraca jako `REG_EXPAND_SZ`. Poprzednia wartość ze znakami sterującymi nie przejdzie już W3-05 przy cofaniu (odmowa, wartość agentki zostaje). | analiza (FFI Windows, bez maszyny Windows) | propozycja: port zwraca `(typ, wartość)`; zapis nad wartością o typie nieprzywracalnym (albo ze znakami sterującymi) — odmowa przed zapisem (fail-closed) |
| W3-11 | platform-windows-sys-impl (`DiskDownloads`) | 2,0 niska | **Okno TOCTOU kwarantanny pobrań.** Brak dowiązań sprawdzany raz w `begin`; `commit` (minuty później) zamyka plik, pisze MOTW i tworzy dowiązanie twarde po **ścieżce** katalogu. Proces agentki (powłoka) może w tym oknie podmienić `Kwarantanna` na junction. Skutek ograniczony (napastnik musi sam podłożyć plik `.part`, a powłoka i tak pisze tam, gdzie konto) — obrona w głąb. | analiza | propozycja: ponowne `checked_dir` tuż przed `place()` i porównanie ścieżki rzeczywistej wyniku z kwarantanną (usunięcie przy niezgodności); docelowo operacje względem otwartego uchwytu katalogu |
| W3-12 | app-files (`attach`), lib-media (`StdFiles`) | 3,0 niska | **Sprawdzenie ścieżki ≠ otwarty plik.** `PathGuard::check` (deny-lista, poświadczenia, po dowiązaniach) i `copy_capped` otwierają ścieżkę osobno — podmiana pliku na dowiązanie w oknie między nimi omija deny-listę; dowiązanie twarde do pliku z deny-listy (P-04) przechodzi zawsze. To samo w nowych narzędziach plikowych fali 3 (`media_*`, `vision_*` z plikiem). | analiza | propozycja (rozszerzenie P-04): po otwarciu sprawdzać uchwyt — `GetFinalPathNameByHandleW` przeciw deny-liście i `nNumberOfLinks > 1` → odmowa/zgoda (FFI w `platform-windows-*`) |

Przejrzane bez ustaleń:
- `tools-system` / `platform-windows-sys-impl::procs_win`: zakończenie procesu — obraz z argumentu = obraz z listy,
  strażnik celów z łańcuchem przodków z tej samej migawki, ścieżka obrazu w katalogach Alfy, procesy krytyczne, PID ≤ 4,
  tylko własne i niepodniesione (SID tokenu + `TokenElevation` na **tym samym** uchwycie `PROCESS_TERMINATE`), tożsamość
  (obraz + czas startu) sprawdzana na uchwycie tuż przed `TerminateProcess` — ponowne użycie PID-u w czasie zgody
  właściciela daje `Changed`; Broker `gui.control(<obraz>)` blokuje obrazy Jądra niezależnie. Uchwyty RAII
  (`OwnedHandle`, `Sc`, `Evt`, `Key`), bufory `TOKEN_USER` wyrównane do `u64`, limity rozmiarów (SID ≤ 4 KiB,
  XML zdarzenia ≤ 1 MiB, komunikat ≤ 64 Ki znaków, wartości rejestru ≤ 128 KiB, ≤ 64 porcje usług, ≤ 4096 zmiennych).
- Usługi: nazwa kanoniczna z SCM, krytyczne i Jądra — tylko start (narzędzie, port, Broker `PROTECTED_SERVICES`),
  `system.admin` = zgoda przy każdej eskalacji. Dziennik: tylko `Application`/`System` (enum), XPath z wartości
  z listy znaków (bez cudzysłowów/nawiasów), parser XML dekoduje encje po wycięciu wartości; komunikaty redagowane.
  Zmienne: wartości o nazwach sekretów nie opuszczają portu, pozostałe redagowane; zdarzenia bez wartości.
- `lib-netguard`/`tools-net`: host i literał IP z parsera WHATWG klienta (wszystkie zapisy liczbowe), IPv4/IPv6
  niepubliczne łącznie z mapowanymi, NAT64, 6to4, Teredo; resolver „wszystkie adresy publiczne” i połączenie na adresy
  z tego samego rozwiązania; bez proxy, bez przekierowań w kliencie — każde przekierowanie przez `redirect_target`
  (bez obniżenia do `http`, limit 5) i nowy host = nowa zgoda Brokera + deny-lista dostawców; limity treści i czasu,
  anulowanie; URL z sekretem → odmowa (heurystyka). `app-plugins` po migracji — testy SR3-02 zielone.
- Kwarantanna: nazwa od serwera oczyszczona (separatory, ADS, urządzenia, kropki/spacje), `.part` jako nowy plik,
  nazwa końcowa przez dowiązanie twarde bez nadpisania, MOTW `ZoneId=3` bez CR/LF w `HostUrl`, porzucenie usuwa część.
- `tools-media`/ffmpeg: argumenty wyłącznie z listy zamkniętej, wymuszony demuxer z nagłówka (bez HLS/concat/wzorców
  `image2` na wejściu), `-protocol_whitelist file`, ścieżki jako `file:` (bez wstrzyknięcia opcji), `-n`, metadane
  usunięte, środowisko bez dziedziczenia, Job Object (2 GiB, 10 min), wynik ≤ 512 MiB, plik tymczasowy usuwany;
  zapis wyniku jako nowy plik przez dziennik cofania (istniejący cel — odmowa, także „pojawił się w trakcie”).
- `lib-media`: odczyty przez `Reader` z budżetem bajtów i kroków, zagnieżdżenie MP4 ograniczone, rozmiary pudełek
  sprawdzane względem rodzica (`size = 0`/`1`), arytmetyka czasu w `u128`; testy właściwości na zmutowanych plikach.
- `tools-vision` / `platform-windows-ocr-impl`: zrzut tylko przez `ScreenCapturePort` z maskowaniem (OCR i model
  dostają ten sam zamaskowany PNG), okno chronione — odmowa przed zrzutem; plik: deny-lista przed Brokerem, wymiary
  z nagłówka przed odczytem całości, dekoder WinRT sprawdza wymiary ponownie przed pikselami (bez `unsafe`);
  prywatność opisu z katalogu sesji (nieznana = tylko lokalnie, bez modelu lokalnego — odmowa); taint przed modelem;
  zdarzenia bez tekstu i pikseli.
- `app-files`: ścieżki przeciągnięcia tylko od powłoki (jednorazowe, 15 s), katalog kopii tylko z natywnego dialogu,
  `backups_verify` tylko dla plików z katalogu kopii, eksport HTML przez `lib-markdown` z CSP `default-src 'none'`
  (tytuł, autorzy, nazwy załączników escapowane), import artefaktów tylko pod `Sesje\Import` (walidacja ścieżek
  wpisów), projekcja załączników dla modelu z porównaniem SHA-256 z chwili wysłania.
- `transfer-contract`: redakcja tur przed sumą `turns_sha256`; ponowna redakcja w `add` jest idempotentna (przy zerze
  trafień zwraca bajty bez zmian), więc suma odpowiada zapisanym bajtom; nagłówek nadal przechodzi przez strażnika.
- `app-agents`: strażnik celów `WinSys` = lista bazowa + bieżący proces (drzewo przez przodków) + katalogi Alfy;
  konwersje bez portu Job Object → „niedostępne” bez pytania o zgodę; prywatność wizji fail-closed.

## 3. Regresje i status poprzednich ustaleń

- Testy `tests/review.rs` z przeglądów #1–#3 w dotkniętych crate'ach przechodzą (`safety-broker-contract` 3/3
  + nowy `#[ignore]`, `app-plugins` 4/4 — SR3-02 po migracji na `lib-netguard`).
- **P-04** (dowiązania twarde do plików z deny-listy) — bez zmian, dotyczy też nowych narzędzi plikowych fali 3
  i załączników (W3-12).
- **P3-01** (trwały stan bezpieczeństwa w Brokerze) — W3-04 pokazuje drugi skutek braku drugiego źródła skażenia:
  flaga sesji z `sessions` nie jest odtwarzana do Brokera nawet w obrębie jednego uruchomienia.
- **P3-04** (sidecary bez przypiętego SHA-256) — dotyczy też `sidecar-ffmpeg` (instalacja ręczna, uruchamiany przez
  Alfę na plikach z Internetu).
- Bez zmian: P-01, P-03, P-05, P-08, P-09, P2-08, P2-10, P3-02, P3-03, P3-05–P3-07.

## 4. Reguły THREAT_MODEL dla kodu fali 3 → testy

| Reguła / próg | Test(y) | Luka |
|---|---|---|
| S11/§7 zabicie procesów Jądra/Alfy | `tools-system-impl/tests/processes.rs`, `platform-apps-contract` (`tests_sys`), `safety-broker-contract/tests/review.rs` (SR3-01) | — |
| S14 SSRF/rebinding | `lib-netguard` (jednostkowe), `tools-net-impl/tests/{net,download}.rs`, `app-plugins/tests/review.rs` | — |
| §5 kanał wyjścia „udział” | **W3-03** `tools-media-impl/tests/review.rs`, `tools-common-contract` (`netpath`) | W3-09 (Broker) |
| S02 injection w pliku → taint | `tools-*` (`report_untrusted`), `app-files/tests/attachments.rs` (flaga sesji) | **W3-04** (flaga nie dociera do Brokera) |
| S26 hasła/okna na zrzutach | `tools-vision-impl/tests/vision.rs` (maskowanie bajt w bajt) | — |
| S19 zatrucie pamięci | — | W3-07 (podmiana kopii) |
| Trwałość poza Brokerem (zmienne) | **W3-02**, **W3-05** `platform-apps-contract/tests/review.rs`, `tools-system-impl/tests/admin.rs` | W3-08 (`PATH`) |
| DoS złośliwym plikiem | **W3-01** `tools-media-impl/tests/review.rs`, `lib-media` (testy właściwości) | — |

## 5. Bramki

Uwaga: w trakcie przeglądu agentka E dzieliła `app-core` (nowy `app-chat`, zmiany w `app-agents`, `app-api`,
`tools-common-contract/src/call.rs`, `tools-system-impl/src/act.rs`) — tych plików nie dotykałam; bramki
uruchamiałam na bieżącym drzewie z jej zmianami w toku.

| Bramka | Wynik |
|---|---|
| `rustfmt --check` (pliki tego przeglądu) | czysto |
| `cargo test` | zielono: `tools-media-impl` (6 binarek; `review` 3/3), `tools-vision-impl` (5 binarek; `review` 1/1), `tools-common-contract` (16 jednostkowych z `netpath`), `platform-apps-contract` (19 + `review` 2/2), `tools-system-{contract,impl,fake}`, `platform-windows-sys-impl`, `safety-broker-contract --test review` (3/3, 1 `ignored`), zależni od `tools-common`: `tools-fs-impl`, `tools-shell-impl`, `tools-office-impl`, `tools-net-impl`; `app-agents --test media --test sysnet`, `app-plugins --test review` (4/4), `lib-media`, `lib-netguard` |
| testy przed poprawką | W3-01, W3-02, W3-03, W3-05, W3-06 — FAILED na kodzie z `HEAD` (komunikaty w tabeli §2) |
| `cargo test … -- --include-ignored` (propozycja Jądra) | W3-09 pada zgodnie z oczekiwaniem (`is_egress() == false`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | czysto |
| `cargo clippy --target x86_64-pc-windows-msvc --all-targets -D warnings` | czysto: `platform-windows-sys-impl`, `platform-windows-ocr-impl`, `platform-apps-contract`, `tools-common-contract`, `tools-media-impl`, `tools-vision-impl` |
| `scripts/check-deps.sh` | OK (1634 krawędzie, zero naruszeń) |
| `cargo deny check` | nie dotyczy — bez zmian zależności |

## 6. Zmienione i nowe ścieżki (do commita przez koordynatora)

Zmienione: `crates/tools-media-impl/src/play.rs`, `crates/platform-apps-contract/src/sys_policy.rs`,
`crates/tools-common-contract/src/{lib.rs,paths.rs}` (w `paths.rs` także przepięty test Q-8),
`crates/tools-vision-impl/src/ocr.rs`, `crates/safety-broker-contract/tests/review.rs` (test `#[ignore]` dopisany na
końcu), `docs/modules/{tools-media,tools-system,tools-common,tools-vision}/SPEC.md` (sekcja „Przegląd fali 3”).
Nowe: `crates/tools-common-contract/src/netpath.rs`, `crates/tools-media-impl/tests/review.rs`,
`crates/tools-vision-impl/tests/review.rs`, `crates/platform-apps-contract/tests/review.rs`,
`docs/reviews/2026-10-wave3-review.md`.

## 7. Decyzje dla człowieka

1. **W3-03 (zmiana zachowania):** narzędzia agentek odmawiają ścieżek sieciowych poza sieciowym katalogiem roboczym
   sesji. Czy dopuszczać udziały z listy właściciela (wtedy razem z W3-09: udział = egress z allowlistą)?
2. **W3-09 (Jądro):** `fs.*` na UNC/WebDAV jako `egress(serwer)` w `derive_facts` Brokera.
3. **W3-04:** podpięcie skażenia z załączników do Brokera w `turns_send` (agentka E/koordynator po podziale
   `app-core`) oraz czy załączone **obrazy** mają skażać sesję (dziś celowo nie; `tools-vision` skaża za ten sam obraz).
4. **W3-07:** katalog kopii zapasowych na deny-liście narzędzi agentek i/lub tylko szyfrowane kopie z harmonogramu.
5. **W3-08:** polityka zapisu `PATH` użytkownika przez agentkę.
6. **W3-02/W3-05:** przegląd rozszerzonej deny-listy zmiennych (polityka bezpieczeństwa narzędzia).
