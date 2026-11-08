# plugin-runtime — SPEC (v1, zaimplementowany)

## Cel
Wtyczki Wasm generowane przez AI albo dostarczone przez właściciela (PLAN §3.2, §8.7, §12.1 pierścień R2, ADR 0012, THREAT_MODEL S09): kod niezaufany w piaskownicy bez dostępu do systemu, z narzędziami trafiającymi do rejestru agentek; jedyny dostęp do świata przez operacje hosta za tokenami Brokera.

## Fala i priorytet
F8, P1. Kryterium ACCEPTANCE F8-05 (limity epoch/fuel/pamięci, brak importów WASI, wtyczka złośliwa zatrzymana).

## Kontrakt (`plugin-runtime-contract`)
```rust
pub struct PluginManifest { id: PluginId, version: semver::Version, author, description, wasm_sha256,
    capabilities: Vec<Capability> /* ⊆ fs.read|fs.write|net.egress */, limits: PluginLimits, tools: Vec<PluginToolDecl> }
pub struct PluginLimits { memory_mib, fuel_per_call, wall_ms, max_input_bytes, max_output_bytes, max_host_calls }  // ≤ ceilings::*
pub struct PluginToolDecl { name /* [a-z][a-z0-9_]{0,39} */, title, description, input_schema, output_schema, mutating }
pub enum PluginState { Proposed, Installed, Disabled, Rejected, Superseded }
pub struct PluginApproval { origin: Ui | Text | Voice /* tylko Ui wystarcza */, reviewed_hash, signature: Option<String> }
#[async_trait] pub trait Plugins { list, installed, tool_catalog, tools /* Vec<Arc<dyn Tool>> */, propose(manifest, wasm, source),
    approve, reject, disable, enable(approval), remove, r2_proposal, deploy_r2(key, value, approval) }
pub enum HostOp { Log{message}, FsReadText{path}, FsWriteText{path, content}, NetGet{url} }   // WIT host.call(op, args-json)
#[async_trait] pub trait PluginHost { async fn execute(&HostOp, Option<&CapToken>, &Holder) -> Result<Value, HostError> }
pub trait PluginStore { load_records, save_records, put_wasm, get_wasm, delete_wasm }   // bajty pod kluczem SHA-256
```
WIT (`wit/alfa-plugin.wit`, pakiet `alfa:plugin@0.1.0`): import `host.call: func(op: string, args: string) -> result<string, string>`, eksport `invoke: func(tool: string, input: string) -> result<string, string>`. Zdarzenia: `plugin.proposed|installed|superseded|rejected|disabled|enabled|removed|load_failed|invoked|trapped|host_denied` — ładunki bez treści wejścia/wyjścia (id, wersja, hashe, stan, paliwo, czas, liczba operacji hosta).

## Zasady
- **Manifest:** zdolności tylko `fs.read`, `fs.write`, `net.egress` (nigdy `system.admin`, `secrets.read`, `shell.exec`, `gui.control` — w v1 także nie wobec obcych aplikacji); zakres poddrzewa ≥ 3 składniki ścieżki, bez udziałów sieciowych; limity dodatnie i ≤ sufitów (64 MiB, 2·10⁹ paliwa, 30 s, 1 MiB I/O, 64 operacje hosta); opisy narzędzi bez znaczników wstrzyknięć (po złożeniu: polskie znaki, znaki niewidoczne); pola nieznane = błąd.
- **Cykl życia jak `skills`:** propozycja (hash bajtów = `wasm_sha256`, nagłówek komponentu, kompilacja i kontrola importów/eksportów) → zatwierdzenie **wyłącznie w oknie** z `reviewed_hash` = SHA-256 kanonicznego manifestu (obejmuje hash modułu, zdolności, limity, opisy) → instalacja; aktualizacja = wyższa wersja i ponowne zatwierdzenie (starsza `Superseded`); wyłączenie; włączenie = ponowne zatwierdzenie; usunięcie kasuje moduły. Bajty są weryfikowane hashem przy zatwierdzeniu i przy każdym ładowaniu (restart); rekord zmieniony poza biblioteką (manifest ≠ zatwierdzony hash) jest nieaktywny.
- **Narzędzia w rejestrze agentek:** nazwa `plugin_<nazwa>` (bez podszywania się pod `fs_read`; kolizja między wtyczkami = odmowa), zdolności `plugin.run` + rodziny z manifestu (koperta `RunGrant`), grupy ról `plugin` i `plugin.<id>`, `mutating`/`reversible: no` przy zapisie lub sieci, **wynik zawsze niezaufany** (`TaintSource::Mcp` — kontrakt Brokera nie ma wariantu „wtyczka”) + zgłoszenie taintu sesji.
- **Piaskownica (impl, wasmtime 36 LTS):** komponent bez importów poza `alfa:plugin/host@0.1.0` (WASI, obce interfejsy, moduły rdzeniowe → odmowa przy ładowaniu), eksport wyłącznie `invoke`; wyłączone memory64, multi-memory, relaxed-SIMD, wątki, GC; na każde wywołanie **nowy `Store` i instancja**; paliwo per wywołanie, przerwanie epokowe (krok 10 ms; czas Wasm bez czasu operacji hosta; anulowanie), limiter pamięci/tabel/instancji, stos 512 KiB, limity wejścia/wyjścia/operacji hosta; równolegle ≤ 4 wywołania; pułapka → `ExecError` (czytelny wynik narzędzia + `plugin.trapped`), nigdy panika.
- **Operacje hosta:** ścisłe parsowanie → zdolność ⊆ manifest (inaczej odmowa bez pytania Brokera) → `BrokerGate::authorize` dla **agentki wywołującej** (jej rola i poziom autonomii; `untrusted_input_in_args = true`, zapis/sieć `reversible: no`) → `verify` → port z tokenem → unieważnienie tokenu. Odmowa wraca do wtyczki jako `err`. `log` bez zdolności — tylko do statystyk (zredagowany, ≤ 16 wpisów).
- **R2 (Ulepszacz):** propozycja = zmiana `plugins.<id z _>.version` (lista `IMPROVABLE`), **wartość = hash przejrzanej wersji** — skrót zatwierdzenia Ulepszacza wiąże dokładne bajty; nigdy automatycznie; wdrożenie `deploy_r2` tylko z zatwierdzeniem tego samego hasha.

## Zależności
`tools-common`, `safety-broker`, `risk-classifier`, `compliance`, `core-bus` (`-contract`); impl: `core-registry-contract`, `wasmtime` (docs/vendor/wasmtime.md).

## Izolacja / budżet
Moduł `inproc`, `lazy`; RAM ≤ 96 MB (silnik + skompilowane moduły) + ≤ 4 × limit pamięci wtyczki; kod wtyczek wyłącznie w piaskownicy, nigdy na wątku RT ani w callbacku audio.

## Integracja (`app-*`, opis)
Zrealizowana w `app-plugins` (komendy, host, problemy dla Diagnosty) + `app-agents` (rejestr narzędzi) + strona Ustawienia → „Wtyczki”; karta R2 pokazuje `r2_proposal` na stronie wtyczek (wdrożenie R2 przez Ulepszacza czeka na weryfikator podpisu TPM — patrz „Otwarte pytania”).
`PluginRuntime::new(PluginDeps { broker, host: <PluginHost: fs przez tools-fs/undo-journal, net przez klient z egress-allowlistą — obie z własnym Broker::verify>, store: DirPluginStore::open(%LOCALAPPDATA%\Alfa\plugins), bus, config })` w rejestrze; `Toolset::tools()` do `RuntimeDeps.tools` agent-runtime (odświeżane po zmianie stanu); rolom, które mają używać wtyczek, dodać grupę `plugin` (personas); komendy UI strony „Wtyczki”: `plugins_list`, `plugins_inspect(bytes)`, `plugins_propose(manifest, bytes)`, `plugins_approve(id, wersja, reviewed_hash)` (tylko z okna, z kartą: zdolności, limity, hash), `plugins_reject`, `plugins_disable`, `plugins_enable(reviewed_hash)`, `plugins_remove`; panel „Zdrowie systemu”: `plugin.trapped`/`load_failed` dla Diagnosty, karta R2 z `r2_proposal`.

## Testy akceptacyjne
- Kontrakt (`contract_tests::lifecycle`) na `-impl` i `-fake`; `tests/contract.rs` (walidacja, operacje hosta, magazyn, obróbka wyniku, hash niezależny od kolejności kluczy), `tests/r2.rs` (strażnik Ulepszacza: R2, nie-auto, skrót zależny od bajtów).
- `plugin-runtime-impl/tests/plugins` (jedna binarka): **87 przypadków negatywnych, 0 ucieczek** — 22 moduły odrzucone przy ładowaniu (WASI fs/sockets/env/clocks/random, obce importy, kształt hosta/eksportu, memory64, wątki, multi-memory, śmieci, obcięcie, rozmiar), 12 manifestów, 20 ataków wykonania (pętle paliwo/czas, bomba pamięci, stos, pułapki, złośliwe ABI wyniku, UTF-8, JSON 5000 poziomów, rozmiar), 20 operacji hosta (zdolność spoza manifestu, `..`, `%VAR%`, urządzenie, poświadczenia CLI → blokada Jądra, odmowa/brak zgody, nieznane operacje, pola dodatkowe, nie-JSON, http, obcy host, limity), 13 osobnych (kanał/hash zgody, podmiana modułu przed/po zatwierdzeniu, brak modułu, podmiana rekordu, pamięć początkowa, wejście, anulowanie, sanitizacja błędu, izolacja instancji, nieaktualne narzędzie). Pozytywnie: licznik słów w rejestrze i w pętli model → narzędzie → model (`providers-fake`), odczyt w zakresie z tokenem (unieważniony, sesja tainted), zapis po zgodzie, magazyn w katalogu po restarcie.

## Fake
`plugin-runtime-fake`: ten sam rdzeń biblioteki, zachowania Rust per hash modułu (domyślnie licznik słów), wirtualny zegar, nagrane zdarzenia i wywołania.

## Otwarte pytania
- `TaintSource::Plugin` w kontrakcie Brokera (przegląd człowieka); podpis zatwierdzeń R2 kluczem TPM (pole `signature`).
- `plugin-sdk` (makra wit-bindgen dla `wasm32-unknown-unknown`) i katalog `plugins/` z WIT — po decyzji o toolchainie autorów; zestaw `evals/F8/wasm-malicious/` do zamrożenia przez człowieka.
- Kompilacja przy każdym starcie (brak bezpiecznej pamięci podręcznej `deserialize`) — zmierzyć na baseline.

## Przegląd bezpieczeństwa #3 (2026-10, `docs/reviews/2026-10-security-review-3.md`)
- **SR3-02 (zrobione w `app-plugins::net`):** host `net.get` sprawdzany parserem WHATWG tego samego klienta (`reqwest::Url`) — nieskanoniczne literały IP (`2130706433`, `0x7f000001`, `0177.0.0.1`, `127.1`, IPv4 zgodny/NAT64 w IPv6) są adresami niepublicznymi; klient łączy się z literałem IP bez resolvera, więc to jedyna kontrola. Reguły sieci wtyczek (adresy publiczne, kanoniczny host i walidacja celu, resolver odrzucający host z adresem niepublicznym, klient bez proxy i przekierowań) pochodzą z `lib-netguard`, wspólnego z `tools-net`.
