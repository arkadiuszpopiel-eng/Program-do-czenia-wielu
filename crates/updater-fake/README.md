# updater-fake

Atrapa modułu `updater`: wersje jako wpisy w pamięci (`install`/`corrupt`), `switch_to` bez restartu, repozytorium
wydań z fixture'ami (`signed_release`, `fake_signature` — dobre i złe podpisy), wirtualny zegar, historia przełączeń,
`fail_next`. Logika stanu z `updater-contract` (ta sama co w `-impl`). Tylko `dev-dependencies` innych modułów.
`FakeFeed` — źródło wydań w pamięci (`ReleaseFeed`): manifesty kanałów, wznawianie od rozmiaru pliku, jednorazowe
przerwanie po N bajtach, brak sieci, dziennik żądań (testy `app-updates` i aplikacji).
