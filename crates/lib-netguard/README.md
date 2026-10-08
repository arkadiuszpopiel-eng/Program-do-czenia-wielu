# lib-netguard

Wspólna biblioteka (bez logiki modułu): ochrona ruchu sieciowego agentek i wtyczek — klasyfikacja adresów publicznych (IPv4/IPv6, mapowane, NAT64, 6to4), walidacja URL parserem WHATWG (tylko https, bez danych logowania i hostów lokalnych), przekierowania bez obniżenia schematu, resolver DNS odrzucający adresy niepubliczne (rebinding) i klient reqwest bez proxy i przekierowań.

Biblioteka pomocnicza (`lib-*`) — bez stanu modułu i bez `module.toml`.

```bash
cargo test -p lib-netguard
```
