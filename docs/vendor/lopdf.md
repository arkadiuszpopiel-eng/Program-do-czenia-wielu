# lopdf — struktura PDF dla tekstu załączników (`app-files`, C2.11)

- Wersja: **`lopdf` 0.45.0** (MIT), `default-features = false` (bez `rayon` i `chrono`). Docs:
  https://docs.rs/lopdf/0.45.0 · repo https://github.com/J-F-Liu/lopdf · zweryfikowano kompilacją,
  testami `cargo test -p app-files` i `cargo deny check` 2026-10-18.
- Zależności przechodnie (wszystkie MIT/Apache/BSD): `aes`, `cbc`, `ecb`, `md-5`, `sha2` 0.11, `rand` 0.10,
  `getrandom` 0.4, `stringprep`, `rangemap`, `weezl`, `brotli-decompressor`, `nom` 8, `indexmap`,
  `encoding_rs`, `flate2`, `thiserror`, `log`. Nowe drugie wersje (`sha2`, `digest`, `cipher`…) — tylko
  ostrzeżenie `multiple-versions`.

## Dlaczego `lopdf`, a nie `pdf-extract`

| | `pdf-extract` 0.12.1 | `lopdf` 0.45.0 + własny interpreter |
|---|---|---|
| Zależności | `lopdf` **0.42** + `adobe-cmap-parser`, `cff-parser`, `euclid`, `postscript`, `type1-encoding-parser`, `unicode-normalization` | sam `lopdf` |
| Limity | brak (bez budżetu operacji, czasu, rozpakowania) | `max_decompressed_size` (0.44+), limity zagnieżdżeń; resztę dokładamy sami |
| Odporność | starszy `lopdf` bez ograniczeń bomb dekompresji z 0.44 | utwardzenia 0.42–0.45 (głębokość nawiasów, drzewo stron, bomby) |

Wybór: `lopdf` do **struktury** (xref, odbudowa uszkodzonego xref, strumienie obiektów, filtry z limitem,
odszyfrowanie RC4/AES z pustym hasłem użytkownika), a **treść stron, czcionki i `ToUnicode` — własny kod**
w `crates/app-files/src/attach/pdf/` (leksykon strumieniowy, interpreter tekstu, parser CMap).

## Używane API
```rust
let options = LoadOptions { password: None, filter: Some(guard), strict: false,
    max_decompressed_size: Some(8 << 20) };                 // limit na pojedynczy strumień obiektów/xref
let doc = Document::load_mem_with_options(bytes, options)?;
doc.is_encrypted();                       // true = hasło użytkownika niepuste (puste odszyfrowuje się samo)
doc.get_pages();                          // BTreeMap<numer od 1, ObjectId> w kolejności drzewa stron
doc.get_page_content_with_limit(id, max)?;// treść strony (wszystkie strumienie) z limitem
doc.dereference(&obj)?;                   // (Option<ObjectId>, &Object), limit łańcucha referencji
stream.get_plain_content_with_limit(max)?; stream.decompress_with_limit(max)?;
font_dict.get_font_encoding(&doc)?.bytes_to_string(&[code])?;  // tylko tabele bazowe i nazwy glifów
```

## Pułapki (sprawdzone w źródłach 0.45.0)
- **Filtr wczytywania** (`FilterFunc`) to wskaźnik funkcji — stan budżetu trzymamy w `thread_local`.
  Dla obiektów najwyższego poziomu `lopdf` ignoruje zwrócony obiekt (używa zmienionego w miejscu), dla obiektów
  ze strumieni obiektów używa zwróconego — dlatego filtr zwraca kopię. Filtr **nie działa** na ścieżce
  dokumentów zaszyfrowanych (tam tylko limit na strumień).
- **`ToUnicode` w `lopdf` omijamy**: zakres `<00000000> <FFFFFFFF>` jest rozwijany element po elemencie
  (zawieszenie, pamięć), pusty cel `<>` kończy się paniką (`last_mut().unwrap()`). Mamy własny parser
  z zakresami nierozwiniętymi.
- `get_font_encoding` przy `Encoding` + `ToUnicode` bierze `Encoding`; jedna nieznana nazwa glifu
  w `Differences` (np. `g12`, `uni0105`) psuje całą tablicę → fallback StandardEncoding. Dlatego `Differences`
  rozwijamy sami (nazwa po nazwie, `uniXXXX`, `uXXXX`, `a.sc`, `f_f`).
- `Content::decode` buduje cały wektor operacji (pamięć ~15× treści) — używamy własnego leksykonu.
- `lopdf` loguje przez `log` — w Alfie bez mostu do `tracing`, więc ostrzeżenia znikają (to dobrze:
  zawierają fragmenty słowników PDF).
- Build wydaniowy powłoki ma `panic = "abort"`: panika w `lopdf` na wrogim pliku zamknęłaby Alfę. Ścieżki,
  których używamy (wczytywanie, filtry, odszyfrowanie), przejrzane; ryzyko szczątkowe opisane w C2.11.
