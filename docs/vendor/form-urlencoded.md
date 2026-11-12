# form_urlencoded1.2.2

Wersja istnieje już w Cargo.lock jako zależność url2.5.8. tools-browser-contract
używa jej bezpośrednio do kodowania parametru q, bez zmiany wersji/źródeł/checksum.
LicencjeMIT/Apache-2.0; źródło crates.io, dokumentacja https://docs.rs/form_urlencoded/1.2.2/.

Sprawdzone na źródle przypiętej paczki: `Serializer::new(String::new())`,
`append_pair("q", text)` oraz `finish()` zwracające tekst query. Spacja jest +,
znakiUTF8 i znaki składniURL są kodowane procentowo. `parse(bytes)` pozwala sprawdzić,
że zapytanie pozostaje jedną wartością q i nie wstrzykuje dodatkowych parametrów.
Biblioteka nie wykonuje żądań sieciowych.
