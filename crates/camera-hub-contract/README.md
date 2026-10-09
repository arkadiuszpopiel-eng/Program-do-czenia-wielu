# camera-hub-contract

Kontrakt modułu `camera-hub` (docs/modules/camera-hub/SPEC.md, plan v6 grupa C3; wersja v0: kamery USB).
`CameraHub` to jedyne miejsce, które otwiera kamery: `state`, `set_master` (przełącznik główny,
domyślnie wyłączony), `set_enabled` (przełącznik kamery), `rename`, `frame` (jedna bieżąca klatka
RGBA, dłuższy bok ≤ `max_side`), `stop_all` (kill-switch), `refresh` (ponowne wyliczenie urządzeń)
i `changes` (`tokio::sync::watch` ze stanem po każdej zmianie). Typy: `CameraId`, `CameraKind`,
`CameraState` (`Asleep` / `Opening` / `Live` / `Failed` / `Unplugged`), `CameraInfo`, `HubState`,
`Frame`, `CameraError`. Trwałość jest wstrzykiwana: `CameraStore` (`StoredCameras` = przełącznik
główny + nazwa i przełącznik każdej kamery), z gotowym `MemoryCameraStore` do testów.
Testy kontraktowe pod feature `contract-tests` uruchamiają te same niezmienniki na `-impl` i `-fake`.
Kontrakt nie zawiera sekretów ani zapisu obrazu na dysk.
