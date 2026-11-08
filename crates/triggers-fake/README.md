# triggers-fake

Atrapa wyzwalaczy: ten sam `TriggersCore`, wirtualny zegar (`advance` wyzwala po kolei terminy,
`jump` — przeskok bez wyzwalania, test zaległości), zadania nagrywane zamiast schedulera
(`set_reject` — odmowa), zdarzenia nagrywane, `restart()` ze stanu, obserwowane katalogi. Tylko jako
dev-dependency. Testy: kontrakt, właściwości crona w strefach z DST (`cron_props.rs`), rdzeń (`engine.rs`).
