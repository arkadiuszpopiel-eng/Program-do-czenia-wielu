# Ponowiony pełny zakres Rust po poprawkach #26–33

Źródła `988d13f366293240e390479d7c3d2c72ac03a099`, Linux Cloud x86_64, Rust1.94.1.
[Wyniki wszystkich poleceń](artifacts/rust-rerun-2026-11-08.json).

**3485 zaliczonych / 1 nieudany / 24 pominięte. Cały zakres wykonany, wynik nie jest zielony.**

| Zakres | Zaliczone | Nieudane | Pominięte |
| --- | ---: | ---: | ---: |
| Workspace bez app-core, z doctestami | 3282 | 0 | 22 |
| app-core: lib, wszystkie76 celów integracyjnych i doctesty | 203 | 1 | 2 |

Polecenia: `cargo test --locked --workspace --exclude app-core --no-fail-fast -j2`, następnie
app-core lib, cele integracyjne w19partiach po4 i doctesty. Profil test/dev debug0,
`CARGO_INCREMENTAL=0`, cel `/workspace/target-nightly`. `cargo clean -p app-core` przed i pomiędzy
partiami mieścił artefakty na dysku32GB. Sterownik i logi: `/workspace/scratch/rust-rerun`,
`run-rust-rerun.py`. Kod0 sterownika nie był traktowany jako zaliczenie — jedno polecenie ma101.

Jedyna porażka: `review::queued_message_of_a_trashed_session_waits_for_undo`, partia14,
`review.rs:114`: licznik kolejki0 zamiast1. Karta [#34](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/34).
Testowy harness ma300ms na Cofnij; kod po wygaśnięciu celowo usuwa sesję i kolejkę. Trwa
weryfikacja wpływu rzeczywistego czasu operacji na test. Nie wydłużano tego okna, timeoutów ani
nie usuwano asercji. Pierwotna próba miała
**3461/20/24** na9b8b7416 i pozostaje osobnym artefaktem.

Kompilator nadal ostrzega o `KillReason`/`KillSwitch` w chronionym teście Brokera.
[Łatka do przeglądu człowieka](patches/linux-broker-test-imports.patch) nie jest zastosowana.
Pełny Clippy z `-D warnings` pozostaje zablokowany; zielone testy części workspace nie oznaczają
zielonego CI. Windows, ręczna zgoda Brokera, prawdziwy mikrofon i model lokalny pomijane
w zwykłych testach wymagają odrębnych przebiegów. Wynik nie jest odbiorem sprzętowym.
