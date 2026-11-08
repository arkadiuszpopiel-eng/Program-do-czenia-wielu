# voice-vad-fake

Atrapa `voice-vad`: deterministyczny VAD energetyczny (okna 10 ms) albo `FakeVad::scripted(przedziały)` —
mowa dokładnie w zadanych przedziałach na osi czasu (wirtualny czas = znaczniki ramek). Wspólny `VadMachine`
z kontraktu. Testy: kontrakt współdzielony, skrypt, wyjście `voice-dsp-fake`.
