# safety-broker — SPEC (v1: logika zaimplementowana — część 1; usługa Windows — część 2)

## Cel
Usługa Brokera na osobnym koncie Windows: wydaje **tokeny zdolności** (`fs.read/write(zakres)`, `shell.exec(zakres)`, `gui.control(app)`, `net.egress(host)`, `secrets.read(id)`, `system.admin(op)`; TTL; potomek ≤ rodzic), prowadzi zatwierdzenia, jest **jedynym writerem Audytu** (łańcuch hashy, kotwica), trzyma polityki Jądra, poziomy autonomii L0–L4, kill-switch (PLAN §8.1–8.4, §8.6). Logika bez UI — okno zatwierdzeń to `broker-ui`.

## Fala i priorytet
F3, P0. Część 1 (ten stan): kontrakt, silnik, Audyt, IPC na dowolnym strumieniu, atrapa. Część 2: binarka usługi na osobnym koncie, named pipe z ACL na SID (przez `platform-windows`), weryfikacja SID/Authenticode klienta, natywne Broker-UI na wyższym poziomie integralności, test odrzucenia SendInput. SPEC i PR-y przez właściciela.

## Kontrakt (źródło prawdy: `crates/safety-broker-contract`)
- `Capability` + zakresy `PathScope` (normalizacja jak `compliance-contract`), `HostPattern` (`*.x.y`, bez `*`/`*.com`), `AppSelector` (bez wieloznaczności), `SecretId`, `AdminOp`; `is_subset_of` tylko w obrębie rodziny.
- `CapToken {id, parent, cap, holder{session, agent, role}, boot, key_epoch, issued_at_ms, expires_at_ms, mac}` — format `ALFT` v1, MAC HMAC-SHA256 na końcu, parser ścisły (re-encode = te same bajty).
- `Broker`: `decide(ActionRequest) -> Decision{Allow(token)|NeedsApproval(ticket)|Deny(reason)}`, `verify(token, needed, presenter)`, `attenuate`, `revoke`, `revoke_holder` (zmiana obsady), `report_untrusted_input`, `submit_plan`, `approval_status`, `request_autonomy_change`, `request_policy_change`, `metrics`.
- `ApprovalChannel` (wyłącznie Broker-UI): `pending() -> [ApprovalChallenge{request, nonce}]`, `resolve(id, decision, PhysicalInputProof)`.
- `PhysicalInputProof`: prywatne pola, brak `Clone/Default/Deserialize`, konstruktor `broker_ui_only::physical_input_proof` (umowa jak `KernelAuthority`; doctesty `compile_fail`). Siłę daje Broker: jednorazowy nonce wysyłany tylko kanałem Broker-UI, wejście niewstrzyknięte, świeżość, opcjonalnie Hello.
- `KernelGuard`/`check_command`, `KernelPolicy`, `AnchorStore`, `ipc::{Hello, Request, Response, ClientRole}`; `KillSwitch`/`JobRegistry` z `watchdog-contract`.
Zdarzenia (Audyt): `broker.token.issued/revoked/denied`, `broker.approval.requested/decided`, `broker.autonomy.changed`, `broker.kernel_block`, `broker.kill_switch`, `broker.policy.changed`, `broker.session.tainted`, `broker.key.rotated`, `broker.audit.chain_started`.

## Zależności
`risk-classifier-contract`, `compliance-contract` (deny-listy, normalizacja), `watchdog-contract` (kill-switch, Job Objects, zegar), `core-bus/log/registry-contract`, `platform-contract`. Krypto: `hmac 0.12.1`, `sha2 0.10.9`, `getrandom 0.4.3`.

## Niezmienniki
- Potomek ⊆ rodzic (zakres, rodzina, TTL ≤ rodzic, ta sama sesja i agentka — delegacja do innej agentki wymaga nowej decyzji); TTL zawsze skończony (domyślnie 30 min, max 4 h).
- Token związany z podmiotem, uruchomieniem (`boot`) i epoką klucza; klucz tylko w pamięci, rotacja z oknem łaski = max TTL; kill-switch: nowy klucz bez łaski + czyszczenie rejestru, zgód, planów, próśb.
- Reguły Jądra sprawdzane przy wydaniu (zakres wewnątrz obszaru chronionego) i przy **każdym użyciu** (konkretna ścieżka/host), na każdym poziomie, także L4.
- Fail-closed: bez zapisu w Audycie nie ma tokenu ani prośby; odmowy i kill-switch działają także bez Audytu.
- Podniesienie poziomu i zmiana polityk wyłącznie przez `ApprovalChannel` z dowodem; agentka → `KernelBlock(SelfEscalation | KernelPolicyChange)` bez tworzenia prośby. Obniżenie działa od razu; obniżenie „na czas” jest bezterminowe, jeśli po wygaśnięciu poziom byłby wyższy niż przed żądaniem (regresja w `tests/negative.rs`); termin w przeszłości = błąd.
- „Zawsze zezwalaj” pokrywa tylko reguły zależne od poziomu (nigdy: głos, taint, trifecta, admin, Jądro), ma limit 24 h, nie zmienia poziomu. Plan pokrywa akcje ⊆ krok, nie groźniejsze niż zadeklarowane, tego samego podmiotu i źródła.
- Taint sesji monotoniczny (zdejmuje go tylko nowa sesja); składnik A trifecty = wydany `fs.read`/`secrets.read`/`shell.exec`.
- Brak nasłuchu TCP; IPC: poświadczenie klienta (rola + termin + MAC), uprawnienia per rola, źródło zmian ustalane z roli.

## Zdolności / uprawnienia
Źródło zdolności; `system.admin` przez UAC na żądanie (część 2).

## Izolacja
`process` (usługa Windows, sesja 0, osobne konto), `always`.

## Budżet zasobów
RAM ≤ 15 MB; `verify` ≤ 0,2 ms; `issue` bez zatwierdzenia ≤ 5 ms; zapis Audytu ≤ 2 ms p95; kill-switch (logika) zmierzony: p95 ≈ 0,1 ms na atrapach.

## Konfiguracja (klucze TOML)
`kernel_policy` = `KernelPolicy`: `token_ttl_default_ms`, `token_ttl_max_ms`, `approval_ttl_ms`, `grant_max_ms`, `plan_ttl_max_ms`, `profile_roots`, `allowed_apps`, `egress_allowlist`, `extra_protected_processes`, `kernel_paths`, `system_drive`, `deny_lists`, `risk`, `hello_required_for`. Procesy i usługi Jądra (`PROTECTED_*`) i obowiązkowe deny-listy nie do usunięcia.

## Testy akceptacyjne
- `ACC-F3-safety-broker-01`: kill-switch < 200 ms p95 z 50 prób (logika: `tests/budget.rs`; prawdziwy system — część 2, CI self-hosted).
- `ACC-F3-safety-broker-02`: „agentka zmienia Jądro / zatwierdza sama siebie” — logika: 116 scenariuszy = 0 sukcesów (`tests/negative.rs`, `tests/ipc.rs`); SendInput do Broker-UI — część 2.
- `ACC-F3-safety-broker-03`: property — potomek nigdy szerszy, zmiana dowolnego bajtu wykrywana, wygasły/obcy token odrzucony (`tests/props.rs`, testy kontraktowe).
- `ACC-F3-safety-broker-04`: łańcuch Audytu weryfikowalny po 10 000 zdarzeń, manipulacje i ucięcie ogona wykrywane kotwicą (`tests/audit.rs`); ACL pliku — część 2.

## Fake
`safety-broker-fake`: prawdziwy silnik z kluczem z jawnego ziarna, Audyt w pamięci, cisza audio jako zdarzenie, skrypt `Allow/NeedsApproval/Deny` per narzędzie (blokady Jądra nie do zdjęcia), `auto_approve` jak `broker-ui-fake`.

## Otwarte pytania
- Kotwica: plik pod ACL konta usługi (część 2) vs TPM — ADR (THREAT_MODEL §11).
- Źródło polecenia (`CommandOrigin`) deklaruje jądro; Broker utwardza je własnym taintem — pełna niezależność po przeniesieniu `voice-cmd` → Broker (F5).
- Strażnik poleceń powłoki jest leksykalny (obrona w głąb obok ograniczonego tokenu procesu); polecenia zakodowane (`-EncodedCommand`, `iex`) blokowane jako nieczytelne.
