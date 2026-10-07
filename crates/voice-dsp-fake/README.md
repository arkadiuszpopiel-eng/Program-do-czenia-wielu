# voice-dsp-fake

Atrapa `voice-dsp`: przepuszcza mikrofon (resampling do 16 kHz mono, ramki 10 ms z czasem) i zwraca
**zaprogramowane** `aec_confidence` / `speech_prob` według skryptu na osi czasu (`ScriptSegment`), referencję
oznacza jako aktywną w przedziałach z `push_reference`; `set_calibration` — wynik kalibracji. Do deterministycznych
testów `voice-dialog`/`voice-vad`. Testy: kontrakt współdzielony + skrypt.
