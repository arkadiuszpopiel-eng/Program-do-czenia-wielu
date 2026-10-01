# broker-ui-impl

Broker-UI (docs/modules/broker-ui/SPEC.md): `NativeBrokerUi<S: ApprovalSurfacePort>` — kolejka kart, sesja karty
(`session::CardSession` — **jedyne** miejsce produkcyjne, gdzie powstaje `PhysicalInputProof`, po regułach
`check_input`; nonce zużywany raz; nowa karta w aktywnym oknie odlicza 500 ms od nowa), widok (`view_of`:
„Odmów” pierwsza i z fokusem, `Esc` = odmowa, `Enter` niczego nie zatwierdza, kolor ryzyka z ikoną i tekstem,
alertdialog tylko przy wysokim ryzyku), `driver::cycle/run` (synchronizacja z Brokerem, wycofywanie kart),
łącza `ChannelLink` (w procesie) i `PipeLink` (named pipe, bilet ze stdin, sprawdzenie konta serwera).
Okno Win32 jest w `platform-windows-impl::WinApprovalSurface`, binarka `alfa-broker-ui` w `app-safety`.
Testy: pełny cykl z prawdziwym silnikiem (`safety-broker-fake`) — clickjacking, wstrzyknięcia, nakładka,
Esc, „zawsze w zakresie”, wygaśnięcie, Hello (`tests/flow.rs`); własności: 2000 ciągów zdarzeń z wejściem
wstrzykniętym = 0 decyzji, każda decyzja z wejścia fizycznego ≥ 500 ms (`tests/props.rs`); `PipeLink` na atrapie.
