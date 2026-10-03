# tools-browser — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Przeglądarka dla agentek (PLAN §7.2 „CDP, własny profil”): Edge/Chrome uruchamiany przez Alfę z **osobnym profilem** i CDP przez **potok**, otwieranie stron po zgodzie Brokera na host, odczyt drzewa dostępności i tekstu, kliknięcia, wpisywanie (bez haseł), zrzut, pobrania w kwarantannie. Nie korzysta z profilu użytkownika, jego ciasteczek ani haseł; sesje zalogowane za zgodą i rozszerzenie — v2.

## Fala i priorytet
F6, P1.

## Kontrakt
```rust
browser_open { url, extra_hosts? } → PageOut { url, title, blocked_hosts, downloads, approved_hosts }
browser_read { max_nodes?, max_chars? } → ReadOutput { page, nodes: [NodeOut{node,depth,role,name,value?,password}], text, truncated }
browser_click { node } → PageOut          browser_type { node, text, submit? } → PageOut
browser_screenshot { max_side? } → obraz PNG     browser_close {} → zamknięcie, zgody hostów wygasają
pub struct BrowserTools; impl BrowserTools { pub fn new(BrowserToolsDeps { browser: BrowserPort, broker, spec, deny, env, config, bus });
                                             pub fn close_all(&self) -> usize /* kill-switch */ }
```
Zdarzenia: `tool.browser.open` (host, liczba zablokowanych), `tool.browser.act` (narzędzie, host), `tool.browser.download` (ścieżka, rozmiar) — bez treści stron.

## Zależności
`tools-common-contract`, `platform-apps-contract` (`BrowserPort`, `EgressFilter`), `safety-broker-contract`, `compliance-contract` (deny-lista domen dostawców), `core-bus-contract`.

## Niezmienniki
- **Egress tylko przez Brokera:** filtr sesji przepuszcza wyłącznie hosty z tokenem `net.egress(host)` i nigdy domen z deny-listy dostawców (claude.ai, chatgpt.com… — THREAT_MODEL S16); `browser_open` pyta o host adresu przy **każdym** wywołaniu (decyzja widzi bieżący taint), `extra_hosts` — każdy osobno; reszta żądań blokowana per żądanie i raportowana (`blocked_hosts`).
- `browser_click`/`browser_type` mogą wysłać dane → `net.egress(host bieżącej strony)` przy każdej akcji (trifecta i taint w Brokerze).
- Adresy tylko `http(s)` z hostem, bez danych logowania w URL; `file:`, `javascript:`, `chrome:`, `edge:` — odmowa.
- Profil Alfy (nigdy profil użytkownika), CDP przez potok, bez haseł i autouzupełniania; pola haseł bez wartości, wpisywanie w nie → odmowa.
- Treść stron **niezaufana** (`untrusted = Web`, taint sesji), sekrety redagowane; pobrania tylko w kwarantannie (plik niezaufany).

## Zdolności / uprawnienia
`net.egress(host)`; odwracalność `no` dla otwarcia i akcji (skutki sieciowe), `yes` dla odczytu i zrzutu (bez Brokera — treść już dopuszczona, taint zgłaszany). Grupy ról: `browser`, `browser.read`, `browser.act`.

## Izolacja
`inproc`, `lazy`; przeglądarka per sesja rozmowy w osobnym procesie (Job Object).

## Budżet zasobów
RAM ≤ 16 MB w procesie Alfy (przeglądarka osobno); ≤ 32 hosty na sesję; wynik dla modelu ≤ 40 000 znaków.

## Konfiguracja (klucze TOML)
`[tools.browser] max_nodes = 300`, `max_chars = 20000`, `max_hosts = 32`; `[platform.browser] kind = "edge"`, `profile_dir`, `quarantine_dir`, `headless = true`.

## Testy akceptacyjne
- `ACC-F6-tools-browser-01`: każde przepuszczone żądanie trafia do hosta z tokenem Brokera; host spoza zgody zablokowany (`tests/browser.rs`).
- `ACC-F6-tools-browser-02`: profil użytkownika nigdy; CDP przez potok; pola haseł bez wartości; pobrania w kwarantannie.
- `ACC-F6-tools-browser-03`: odmowa Brokera / deny-lista dostawców → zero ruchu sieciowego i brak uruchomienia przeglądarki.
- F6-05 (URL dostawców) i F6-01 (kategoria „przeglądarka”) — VM, self-hosted.

## Fake
`tools-browser-fake` (manifesty, walidacja, wyniki skryptowane); testy impl na `platform-apps-fake::FakeBrowser`.

## Otwarte pytania
- Ruch poza domeną `Fetch` (WebRTC, WebTransport) — lokalne proxy egressu w helperze (v2).
- Sesje zalogowane za zgodą (osobny profil per serwis) i rozszerzenie Alfy — THREAT_MODEL §11.
