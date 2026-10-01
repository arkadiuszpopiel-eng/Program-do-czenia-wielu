# voice-stt-fake

Atrapa `voice-stt`: transkrypty z adnotacji (`script(text)`), partiale = prefiks słów proporcjonalny do audio,
deterministyczne znaczniki słów, opóźnienie wirtualne (`set_latency_ms`), awaria GPU (`crash_next` → zdarzenie
fallback, final z CPU bez utraty wypowiedzi). Bramka VAD i prywatność z kontraktu.
