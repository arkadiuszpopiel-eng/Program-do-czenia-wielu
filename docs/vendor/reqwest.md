# reqwest (klient HTTP adapterów `providers-api-impl`)

- Wersja: **0.12.28** (przypięta w `crates/providers-api-impl/Cargo.toml`), docs: https://docs.rs/reqwest/0.12.28
- Zweryfikowano kompilacją i testami: 2026-09-30.

## Dlaczego 0.12, a nie 0.13
W 0.13 feature `rustls` włącza dostawcę kryptografii **aws-lc-rs** (`aws-lc-sys` ma licencję z członem
`OpenSSL` — poza allowlistą `deny.toml`, wymaga też CMake przy budowie na Windows). W 0.12 feature
`rustls-tls-native-roots` = rustls + **ring** (Apache-2.0/ISC) + certyfikaty systemu (Windows: magazyn
certyfikatów przez `rustls-native-certs`/`schannel`). Bez `webpki-roots` (licencja CDLA).

## Używane feature'y
`default-features = false`, `["rustls-tls-native-roots", "stream", "http2"]`. **Nie** włączamy
`system-proxy` (ciągnie `windows-registry` — reguła „jeden windows-rs") ani `json` (ciało serializujemy
sami: `.body(serde_json::to_vec(..))` + nagłówek `content-type`).

## Używane API
```rust
let client = reqwest::Client::builder().connect_timeout(d).pool_idle_timeout(d).build()?;
let resp = client.post(url).headers(h).body(bytes).send().await?;  // Future z nagłówkami odpowiedzi
resp.status(); resp.headers(); resp.text().await; resp.bytes().await;
let mut body = resp.bytes_stream();                                  // feature `stream`; futures StreamExt::next
err.is_connect(); err.is_timeout();
let mut v = HeaderValue::from_str(key)?; v.set_sensitive(true);      // klucz nie trafia do Debug
```
Upuszczenie `Response`/strumienia bajtów przed końcem zamyka połączenie (HTTP/1.1: FIN; HTTP/2: RST_STREAM)
— na tym opiera się anulowanie ≤ 100 ms.

## Pułapki
- `send()` rozwiązuje się po nagłówkach — limit „first-token" liczymy od wysłania do pierwszego zdarzenia SSE.
- Bez `json` nie ma `RequestBuilder::json`/`Response::json`.
