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
| Odporność | starszy `lopdf` bez ograniczeń bomb dekompresji z 0.44 | utwardzenia 0.42–0.45 (głębokość nawiasów, cykle w drzewie stron, bomby) — ale nie wszędzie; ścieżki bez limitów omijamy (niżej) |

Wybór: `lopdf` do **struktury** (xref, odbudowa uszkodzonego xref, obiekty najwyższego poziomu, filtry z limitem,
algorytmy szyfrowania RC4/AES), a **strumienie obiektów, odszyfrowanie pustym hasłem, treść stron, czcionki
i `ToUnicode` — własny kod** w `crates/app-files/src/attach/pdf/` (`prepare`, `objstm`, `streams`, leksykon
strumieniowy, interpreter tekstu, parser CMap).

## Używane API
```rust
let options = LoadOptions { password: None, filter: Some(guard), strict: false,
    max_decompressed_size: Some(8 << 20) };                 // limit na strumień xref
let doc = Document::load_mem_with_options(&prepared, options)?; // bajty po `prepare` (niżej)
doc.authenticate_password("")?; EncryptionState::decode(&doc, "")?; encryption::decrypt_object(..);
ObjectStream::new_with_limit(&one_object_stream, None)?; // parser jednego obiektu z wycinka
for id in doc.page_iter() { .. }          // tylko `next()` — NIGDY `get_pages()` / `collect()`
doc.get_page_content_with_limit(id, max)?;// treść strony (wszystkie strumienie) z limitem
doc.dereference(&obj)?;                   // (Option<ObjectId>, &Object), limit łańcucha referencji
stream.get_plain_content_with_limit(max)?; stream.decompress_with_limit(max)?;
font_dict.get_font_encoding(&doc)?.bytes_to_string(&[code])?;  // tylko tabele bazowe i nazwy glifów
```

## Pułapki (sprawdzone w źródłach 0.45.0)
- **`get_pages()` zabija proces**: zbiera `page_iter()` do kolekcji, a `size_hint` sumuje `/Count` węzłów `/Pages`
  z pliku — `/Count 100000000000` rezerwuje terabajty (błąd alokacji = koniec procesu, bez względu na wątek),
  `i64::MAX` daje panikę `capacity overflow`. Strony zbieramy pętlą `for` (tylko `next()`) do 1000.
- **`ObjectStream::new*` kopiuje obiekty**: każdy obiekt parsuje od swojego przesunięcia do końca strumienia, więc
  powtórzone przesunięcia (albo zagnieżdżone nawiasy) dają tysiące kopii dużego napisu z kilku KB pliku. `lopdf`
  robi to przy wczytywaniu (`ObjStm`), w ścieżce dokumentów zaszyfrowanych (bez filtra) i przy rozwiązywaniu
  pośredniego `/Length` wskazującego obiekt ze strumienia obiektów. Dlatego: filtr odkłada każdy `ObjStm`
  (zwraca `None`), a `prepare` przed wczytaniem zamienia w bajtach `/Encrypt` → `/AlfaEnc` i pośrednie
  `/Length N G R` → `/AlfaLn` (ta sama długość, także nazwy z `#xx`). Potem sami: treść tych strumieni z surowych
  bajtów (do następnego obiektu, zakończona `endstream`, budżet = rozmiar pliku), odszyfrowanie pustym hasłem,
  strumienie obiektów z budżetem (każdy obiekt tylko ze swojego wycinka).
- **Strumienie zagnieżdżone w sobie** (bezpośrednie `/Length` obejmujące kolejne obiekty) `lopdf` kopiuje osobno —
  suma `/Length` w pliku ponad 2× rozmiar pliku = odmowa przed wczytaniem; filtr liczy surowe bajty strumieni.
- **Filtr wczytywania** (`FilterFunc`) to wskaźnik funkcji — stan budżetu trzymamy w `thread_local`.
  Dla obiektów najwyższego poziomu `lopdf` ignoruje zwrócony obiekt (używa zmienionego w miejscu) — `None` go
  pomija; filtr nie działa na ścieżce dokumentów zaszyfrowanych, dlatego `lopdf` jej nie widzi (`/AlfaEnc`).
- **`ToUnicode` w `lopdf` omijamy**: zakres `<00000000> <FFFFFFFF>` jest rozwijany element po elemencie
  (zawieszenie, pamięć), pusty cel `<>` kończy się paniką (`last_mut().unwrap()`). Mamy własny parser
  z zakresami nierozwiniętymi.
- `get_font_encoding` przy `Encoding` + `ToUnicode` bierze `Encoding`; jedna nieznana nazwa glifu
  w `Differences` (np. `g12`, `uni0105`) psuje całą tablicę → fallback StandardEncoding. Dlatego `Differences`
  rozwijamy sami (nazwa po nazwie, `uniXXXX`, `uXXXX`, `a.sc`, `f_f`).
- `Content::decode` buduje cały wektor operacji (pamięć ~15× treści) — używamy własnego leksykonu.
- `lopdf` loguje przez `log` — w Alfie bez mostu do `tracing`, więc ostrzeżenia znikają (to dobrze:
  zawierają fragmenty słowników PDF).
## Ryzyka szczątkowe
- Build wydaniowy powłoki ma `panic = "abort"`: panika albo błąd alokacji w `lopdf` na wrogim pliku zamknęłaby Alfę
  (osobny wątek tu nie chroni). Ścieżki, których używamy, przejrzane w źródłach 0.45.0 — z poprawkami wyżej;
  przy podnoszeniu wersji przejrzeć je ponownie (szczególnie `reader.rs`, `object_stream.rs`, `document.rs`).
- Czas procesora: `lopdf` czyta każdy wpis xref (także powtórzone przesunięcia tego samego dużego obiektu) zanim
  filtr może go odrzucić, a przerwać wczytywania się nie da. Wrogi plik może więc zająć wątek długo po terminie
  10 s; wątki ponad termin liczą się do limitu 3 jednoczesnych, potem ekstrakcja PDF odpowiada „zajęte” do
  zakończenia tych wątków (pamięć zostaje ograniczona budżetami).
- Pamięć sparsowanych obiektów jest kilkadziesiąt razy większa niż ich tekst (np. tablica `[0 0 0 …]`): 25 MB pliku
  albo 64 MB rozpakowanych strumieni obiektów to w najgorszym razie ok. 1–3 GB.
