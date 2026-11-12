# Synchronizacja lockfile powłoki

CI192/Tauri130978290522 na5f265ba3: cargo metadata --locked nie może uaktualnić lockfile, kod1.
Cloud odtwarza ten sam błąd, kod101. Cargo metadata --offline dodaje poprawny pakietworkspace,
ale kończy101 z powodu brakującej lokalnej paczki alloc-no-stdlib3.0.0. Następnie pełne
cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --format-version1:
**0**, oficjalne istniejące paczki pobrane zcrates.io, bez dalszej zmianylockfile.

Różnica11dodanych linii: lib-client-wire + krawędź app-api. Wszystkie dotychczasowe wersje,
źródła i checksums identyczne. Nie edytowano lockfile ręcznie i nie usunięto --locked.
To rozwiązywanie zależności naLinux; brak lokalnej natywnej kompilacjiTauri/Windows.
OdrębneCI nowegoheada oraz przegląd pozostają wymagane. Zachowano trwająceCI192;
fix przygotowano na osobnej gałęzi, bez anulowania pozostałych zadań staregoheada.
