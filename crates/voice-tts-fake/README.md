# voice-tts-fake

Atrapa `voice-tts`: „mowa syntetyczna” (F0 = 200 Hz × wysokość presetu) zdanie po zdaniu, deterministyczne
znaczniki słów (`Native`), TTFB wirtualne (`set_ttfb_ms` → zdarzenie `started`), awarie silników
(`fail_engine("pocket")` → fallback na następne ogniwo łańcucha). Głosy v0 z kontraktu.
