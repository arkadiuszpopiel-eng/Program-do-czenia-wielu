# camera-hub-fake

Deterministyczna atrapa `CameraHub` (docs/modules/camera-hub/SPEC.md, sekcja „Fake”) dla modułów
biorących klatki (`vision-describe`, `person-recognition`) i testów interfejsu — bez sprzętu,
portu systemowego i zegara. `FakeCameraHub::new(n)` daje `n` kamer `fake-cam-0…` z ruchomym wzorem
testowym (numer klatki zmienia piksele, każda kamera ma inny kolor). Test steruje zdarzeniami:
`unplug` / `plug` (powrót widać po `refresh`), `fail` (błąd żądań klatki jednej kamery),
`elapse_idle` (upływ okresu bezczynności — kamery zasypiają) i czyta `open_streams()`.
Reguły przełączników (główny i per kamera) są takie same jak w `-impl`; sprawdza to wspólny
zestaw `camera_hub_contract::contract_tests`.
