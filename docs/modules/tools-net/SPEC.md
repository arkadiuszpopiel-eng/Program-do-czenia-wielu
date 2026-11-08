# tools-net — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Sieć dla agentek bez przeglądarki (PLAN §7.2 „Sieć”: HTTP, pobieranie, wyszukiwanie): `net_fetch` (GET/HEAD), `net_download` (plik do kwarantanny sesji z SHA-256 i MOTW) i `net_search` jako port bez dostawcy (do podpięcia później). WebSocket, POST/formularze, sesje zalogowane — v2 (POST to kanał wysyłki — wymaga osobnej analizy trifecty).

## Fala i priorytet
F6, P1.

## Kontrakt
```rust
net_fetch { url, method?: get|head, max_chars? } → FetchOut { url, final_url, status, content_type?, bytes, text?, truncated, redirects: [url] }
net_download { url, file_name? } → DownloadOut { path, bytes, sha256, content_type?, final_url, executable }
net_search { query, max_results? } → { results: [SearchHit{title, url, snippet}] }   // tylko gdy dostawca jest podpięty
pub struct NetTools; impl NetTools { new(NetToolsDeps{ http: HttpPort, search: Option<SearchPort>, downloads: DownloadStore, quarantine_root, broker, deny, env, config, bus }) }
#[async_trait] trait HttpPort { send(HttpRequest) -> HttpResponse { status, content_type, content_length, location, body: BodyReader } }   // bez przekierowań
#[async_trait] trait SearchPort { endpoint_host() -> Option<String>; search(query, max) -> Vec<SearchHit> }
```
Zdarzenia: `tool.net.fetch` (host, status, bajty), `tool.net.download` (ścieżka, bajty, SHA-256), `tool.net.redirect` (host źródłowy → docelowy) — bez treści.

## Zależności
`tools-common-contract`, `lib-netguard` (klasyfikacja adresów, walidacja URL parserem WHATWG tym samym co klient, resolver z odrzucaniem adresów niepublicznych, klient `reqwest` bez proxy i przekierowań), `platform-apps-contract` (`DownloadStore`), `safety-broker-contract`, `compliance-contract` (deny-lista domen dostawców), `core-bus-contract`. Kwarantanna Windows: `platform-windows-sys-impl::DiskDownloads`; atrapa: `platform-apps-fake::FakeDownloads`.

## Niezmienniki
- **Egress tylko przez Brokera:** każde wywołanie prosi o `net.egress(host)` (decyzja widzi bieżący taint, trifectę i allowlistę); przekierowanie na **inny host** = nowa zgoda Brokera, na ten sam host — ta sama zgoda; najwyżej 5 przekierowań; przekierowanie na `http://` (obniżenie) albo adres niepubliczny = odmowa.
- **Tylko `https://`**, bez danych logowania w URL, bez `localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa`, bez adresów IP niepublicznych (pętla, prywatne, link-local, CGNAT, multicast, dokumentacja, `::ffff:` z takim adresem, ULA); domeny dostawców modeli z deny-listy Jądra → odmowa przed Brokerem.
- **DNS rebinding:** resolver klienta odrzuca host, gdy **którykolwiek** rozwiązany adres jest niepubliczny; połączenie idzie wyłącznie na adresy sprawdzone w tym samym rozwiązaniu (bez drugiego zapytania DNS); każde przekierowanie i każde wywołanie rozwiązuje i sprawdza od nowa.
- **Bez proxy systemowego**, bez ciasteczek, bez nagłówków od modelu; adres wyglądający na zawierający sekret (wzorce `tools-common::text`) → odmowa (eksfiltracja przez URL).
- **Limity:** treść `net_fetch` ≤ 2 MiB (przekroczenie = przerwanie odczytu, wynik obcięty), czas całkowity ≤ 30 s; pobieranie ≤ 200 MiB i ≤ 300 s; nagłówek `Content-Length` ponad limit = odmowa bez odczytu treści; anulowanie (kill-switch) przerywa odczyt i usuwa plik częściowy.
- **Treść niezaufana:** `untrusted = Web`, taint sesji zgłaszany Brokerowi, sekrety redagowane, tekst tylko dla typów tekstowych.
- **Pobieranie:** wyłącznie do kwarantanny `<katalog roboczy sesji>\Kwarantanna` (albo `%USERPROFILE%\Alfa\Kwarantanna\<sesja>`), drugi token `fs.write(plik)`; nazwa od serwera oczyszczona (bez separatorów, ADS, nazw urządzeń, kropek/spacji na końcu), plik `.part` tworzony jako nowy i przemianowywany bez nadpisania, katalog i jego przodkowie w kwarantannie nie mogą być dowiązaniem/junction, SHA-256 liczone w locie, MOTW (`Zone.Identifier`, `ZoneId=3`, `HostUrl`) na Windows; rozszerzenia wykonywalne oznaczone w wyniku.

## Zdolności / uprawnienia
`net.egress(host)` (wszystkie), `fs.write(plik w kwarantannie)` (`net_download`); `reversible = no` (skutki sieciowe), mutujące (wysyłka). Grupy ról: `net`, `net.read` (`net_fetch`, `net_search`), `net.download`. W obsadzie: Wykonawczyni (`net`), Badaczka (`net.read`).

## Izolacja
`inproc`, `lazy`; klient HTTPS współdzielony (pula połączeń `reqwest`, rustls z certyfikatami systemu).

## Budżet zasobów
RAM ≤ 8 MB + bufor odpowiedzi `net_fetch` (≤ 2 MiB); pobieranie strumieniowo (fragmenty, bez całego pliku w pamięci).

## Konfiguracja (klucze TOML)
`[tools.net] max_fetch_bytes = 2097152`, `max_download_bytes = 209715200`, `fetch_timeout_ms = 30000`, `download_timeout_ms = 300000`, `max_redirects = 5`, `max_chars = 20000`.

## Testy akceptacyjne
- `ACC-F6-tools-net-01`: każdy kontakt z hostem poprzedzony tokenem `net.egress(host)`; przekierowanie na inny host bez zgody → zero żądań do tego hosta (`tests/net.rs`).
- `ACC-F6-tools-net-02`: ≥ 30 przypadków negatywnych (prywatne IP w każdej postaci, rebinding DNS, przekierowania, ogromne odpowiedzi, `http://`, dane logowania, deny-lista dostawców, nazwy plików, dowiązania w kwarantannie) → odmowa bez skutków.
- `ACC-F6-tools-net-03`: pobranie ma SHA-256 zgodne z treścią, plik tylko w kwarantannie, przerwanie usuwa `.part`.

## Fake
`tools-net-fake` (manifesty, walidacja, wyniki skryptowane); testy impl na `FakeHttp` (wirtualna sieć z przekierowaniami, dziennik żądań) i `platform-apps-fake::FakeDownloads`.

## Otwarte pytania
- Dostawca wyszukiwania (API z kluczem w sejfie; trasa w `compliance`) — port gotowy, brak dostawcy.
- Wspólny klient z `app-plugins` (`EgressClient`) — migracja na `lib-netguard` po przeglądzie bezpieczeństwa #3.
- POST/WebSocket — v2 z analizą trifecty i podglądem wysyłanych danych w Broker-UI.
