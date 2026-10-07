# voice-persona-fake

Atrapa modułu `voice-persona` do testów innych modułów (`voice-tts`, `voice-dialog`, UI):
`FakePersona` implementuje `Persona` bez reguł językowych — biblie głosu to wbudowane fixture'y
z `docs/PERSONAS.md`, normalizator jest tabelowy (wpisy słownika dla pojedynczych słów + cyfry
czytane pojedynczo, więc w mowie nigdy nie zostają cyfry), plan dzieli tekst na linie, kod i tabele
przenosi na ekran, a styl jest zawsze neutralny. Chunker atrapy (`LineChunker`) tnie tylko na końcach
linii. Atrapa rejestruje wywołania `plan` (`planned()`) i przechodzi ten sam test kontraktowy co
`voice-persona-impl`. Zależy wyłącznie od `voice-persona-contract`.
