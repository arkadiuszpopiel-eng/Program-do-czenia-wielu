# remote-relay — SPEC (szkic v0, planowany — plan v6)

## Cel
Osobna usługa w Internecie, która łączy w pary dwa połączenia wychodzące — hosta i klienta — i przenosi między
nimi nieczytelne ramki kanału end-to-end. Przekaźnik **nie** zna treści, nazw, kluczy ani kont, nie
uwierzytelnia użytkowników, nie przechowuje ramek i nie jest częścią hosta ani Jądra. Właściciel może postawić
własny egzemplarz. Stan: propozycja, kod nie istnieje.

## Fala i priorytet
Plan v6, **etap D** (zadania D.3, D.4), po podpisie ADR 0019–0023 (D.0). Priorytet: do ustalenia przez
właściciela. v0: przekazywanie ramek po WebSocket/TLS, limit połączeń, obraz kontenera i instrukcja.
v1: do ustalenia (np. statystyki bez treści); droga bezpośrednia iroh dla klientów natywnych jest poza tym modułem.

## Kontrakt (szkic Rust)
```rust
// szkic — przekaźnik jest binarką; „kontraktem” jest protokół łącza, wspólny z remote-gateway i klientami
pub struct HostId(pub [u8; 16]);                 // losowy, bez związku z kontem ani nazwą
pub enum RelayHello { Host { host_id: HostId }, Client { host_id: HostId } }
pub enum RelayFrame { Opaque(Bytes) }            // szyfrogram kanału Noise — przekaźnik go nie interpretuje
pub struct RelayLimits { pub conns_per_host: u32, pub max_frame_bytes: u32, pub idle_timeout_s: u32 }
pub trait Relay {
    fn accept(&self, hello: RelayHello, peer: PeerAddr) -> Result<ConnId, RelayError>;
    fn forward(&self, from: ConnId, frame: RelayFrame) -> Result<(), RelayError>;
    fn close(&self, conn: ConnId);
}
```
Zdarzenia na magistrali Alfy: brak (usługa poza hostem). Własny dziennik usługi — bez treści ramek.

## Zależności
Żadnych crate'ów modułów Alfy poza wspólnym typem protokołu łącza (do ustalenia: `remote-gateway-contract` albo
osobna biblioteka `lib-*`). Zewnętrzne: serwer WebSocket i TLS (wybór i wersje — `docs/vendor/` przed kodem).

## Niezmienniki
- Przekaźnik przenosi wyłącznie szyfrogram: znany tekst wiadomości nigdy nie występuje w niczym, co przekaźnik
  widzi lub zapisuje.
- Przekaźnik zna tylko: losowy identyfikator hosta, adresy IP stron, rozmiary i czas ramek.
- Nie może podszyć się pod hosta: klient zna klucz publiczny hosta z parowania (R1).
- Ramki nie są zapisywane na dysku ani buforowane dłużej, niż wymaga przekazanie.
- Limit połączeń na identyfikator hosta i limit rozmiaru ramki są egzekwowane przed przekazaniem (R9).
- To **jedyny** składnik planu v6, który nasłuchuje — i działa poza komputerem właściciela; reguła „żadnego
  nasłuchu” dotyczy hosta i pozostaje nienaruszona.
- Awaria lub wyłączenie przekaźnika nie wpływa na pracę lokalną Alfy.

## Zdolności / uprawnienia
Brak tokenów Brokera — usługa nie działa na komputerze właściciela i nie ma dostępu do jego danych.
Na serwerze: tylko port nasłuchu i certyfikat TLS.

## Izolacja
`process` — osobny plik wykonywalny / obraz kontenera na serwerze; cykl życia `always` po stronie serwera.
Nie jest uruchamiany przez hosta.

## Budżet zasobów
RAM na połączenie, liczba równoczesnych połączeń, dodane opóźnienie ramki — do pomiaru; progi ustala właściciel.
Koszt hostingu i domeny — po stronie właściciela.

## Konfiguracja (klucze TOML)
Szkic (plik usługi, nie `%APPDATA%\Alfa`): `listen = "…"`, `tls_cert`, `tls_key`, `conns_per_host = <limit>`,
`max_frame_bytes = <limit>`, `idle_timeout = "…"`, `log_level`. Po stronie hosta tylko `[remote] relay_url`.

## Wkład do UI
Brak własnego interfejsu. W Ustawieniach hosta: adres przekaźnika i stan połączenia (część `remote-gateway`).
Dla właściciela: instrukcja postawienia własnego przekaźnika.

## Testy akceptacyjne
- `ACC-V6-remote-relay-01` — „przekaźnik nie widzi treści”: przekaźnik testowy zapisuje wszystko, co przenosi;
  znany tekst wiadomości nie występuje w zapisie.
- `ACC-V6-remote-relay-02` — przekaźnik podmieniający lub powtarzający ramki nie powoduje wykonania żadnego
  polecenia po stronie hosta (0 skutków w N próbach; N ustala właściciel).
- `ACC-V6-remote-relay-03` — przekroczenie limitu połączeń na identyfikator hosta kończy się odmową.
- `ACC-V6-remote-relay-04` — dodane opóźnienie ramki poniżej progu przy obciążeniu testowym (próg ustala właściciel).
- `ACC-V6-remote-relay-05` — obraz kontenera uruchamia się według instrukcji na czystym serwerze (przebieg ręczny).

## Fake
Przekaźnik w pamięci dla testów hosta i klientów: tryb „zapisuj wszystko” (test treści), tryb złośliwy
(podmiana, powtórzenie, opóźnienie, zrywanie), wirtualny zegar.

## Otwarte pytania
- ADR 0021 — kto prowadzi przekaźnik (własny serwer właściciela czy dostawca chmurowy); dobór wzorca Noise.
- Plan określa moduł jako „osobną usługę”, a zasada 2 planu v6 wymaga trójki crate'ów — do rozstrzygnięcia układ
  crate'ów (binarka + wspólny protokół + atrapa).
- Jak ograniczyć nadużycie publicznego przekaźnika przez obcych bez wprowadzania kont — do ustalenia.
- Czy i jak długo przekaźnik zapisuje adresy IP w dzienniku (prywatność, zgodność prawna) — decyzja właściciela.
- Powiadomienia push „obudź i pobierz” (E.5): czy przechodzą przez przekaźnik, czy osobną drogą — do ustalenia.
