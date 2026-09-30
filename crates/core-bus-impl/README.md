# core-bus-impl

Produkcyjna magistrala zdarzeń in-proc (`BroadcastBus`) na `tokio::sync::broadcast`
z ograniczonym buforem per subskrybent (`BusConfig::buffer_per_subscriber`, domyślnie 4096).
`publish` nigdy nie blokuje wydawcy: wolny subskrybent dostaje `BusItem::Lagged(n)`,
a licznik `BusStats::dropped` rośnie o `n` (backpressure bez blokady, SPEC core-bus).
Filtrowanie odbywa się po stronie subskrypcji, więc każdy subskrybent widzi tylko swoje zdarzenia.
Zależy wyłącznie od `core-bus-contract`; testy: kontraktowe (współdzielone z `-fake`),
wieloproducentowe i test przepełnienia bufora.
