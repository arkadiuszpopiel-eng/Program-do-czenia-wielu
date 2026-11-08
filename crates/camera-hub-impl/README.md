# camera-hub-impl

Implementacja modułu `camera-hub` (docs/modules/camera-hub/SPEC.md, v0: kamery USB).
`CameraHubService::start(port, store, config)` składa hub nad `CameraPort` z `platform-contract`
(Windows: Media Foundation w `platform-windows-impl`; testy: `platform-fake::FakeCameras`) —
moduł nie ma własnego kodu systemowego. Zasady:

- przełącznik główny wyłączony (domyślnie) albo kamera wyłączona ⇒ port nigdy nie dostaje `open`;
- każda kamera ma własne gniazdo strumienia; blokujące wywołania portu idą przez
  `spawn_blocking`, więc kilka kamer działa naraz, a błąd albo odłączenie jednej nie dotyka reszty;
- strumień otwiera pierwsze żądanie klatki, a zamyka zadanie usypiające po `HubConfig::idle_sleep`
  (domyślnie 20 s) bez żądań;
- `stop_all`, wyłączenie kamery i przełącznika głównego podnoszą epokę gniazda: żądania w toku
  kończą się `CameraError::Stopped` i niczego nie otwierają, a strumień zostaje zamknięty;
- ustawienia (przełączniki, nazwy) zapisuje wstrzyknięty `CameraStore`; klatki żyją tylko w pamięci.

Urządzenia są wyliczane przy starcie tylko przy włączonym przełączniku głównym, potem na `refresh`
i przy włączeniu przełącznika. Kamery sieciowe i zdarzenia `camera.*` na magistrali — v1.
