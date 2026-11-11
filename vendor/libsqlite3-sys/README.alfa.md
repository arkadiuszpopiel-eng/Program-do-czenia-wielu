# Adapter SQLCipher Alfy

Źródło: https://static.crates.io/crates/libsqlite3-sys/libsqlite3-sys-0.39.1.crate

SHA256: 14a5f01b87b92705d07b433c06f96cbc764637d81a0bffb5734ce17cf0f9a2b3.

Zmiany: SQLCipher-src420.0.0, rozdzielenie istniejących modułów build.rs,
przeniesienie wiązań bindgen bez zmian tokenów; jawna normalizacja białych znaków w czterech plikach. Obsługiwany jest szyfrowany wariant bundled-sqlcipher; nie dostarczamy nieużywanego
zwykłego silnika SQLite ani obcego Cargo.lock. Bez zmiany polityki źródeł.

Odtworzenie: node scripts/vendor-sqlite.mjs /ścieżka/do/libsqlite3-sys-0.39.1.crate.
Sprawdzenie offline: node scripts/vendor-sqlite.mjs --check.
