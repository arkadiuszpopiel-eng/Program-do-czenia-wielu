# Spike (h) — wyniki: <maszyna> — <RRRR-MM-DD>

Nagłówek (skrypty wypełniają same; uzupełnij, czego brakuje):

| Pole | Wartość |
|---|---|
| Maszyna | desktop / laptop / desktop-emu / laptop-emu |
| CPU / RAM | |
| GPU / VRAM / sterownik | |
| Windows (winver) | |
| whisper.cpp — tag wydania, paczka | |
| llama.cpp — tag wydania (bNNNN), paczka | |
| Modele (plik + SHA-256) | |
| Zasilanie (laptop) | zasilacz / bateria |
| Emulacja | brak / affinity 0xFFF + limit pamięci 12 GB |

## whisper.cpp

| Maszyna | Tryb (emulacja?) | Backend | Model | Wątki | `whisper-bench` enkoder [ms] | Transkrypcja 60 s: mediana [s] | max [s] | RTF | RTF po korekcie (×2,2 GPU-desktop / ×1,25 CPU-desktop) | Szczyt VRAM [MB] | Stabilność: minuty / uruchomienia / crashe | Uwagi |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | | |

Progi: 0 crashy w 1 h; szczyt VRAM ≤ 7168 MB; RTF po korekcie zapisany (cel orientacyjny < 0,3 dla turbo Q5).

## llama.cpp (`llama-bench -p 512 -n 128`)

| Maszyna | Tryb | Backend | Model (plik) | Rozmiar [GB] | `-ngl` | Wątki | pp512 [tok/s] | tg128 [tok/s] | tg128 po korekcie | Szczyt VRAM [MB] | Uwagi (OOM? częściowy offload?) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | |

## Pocket TTS PL (z `bench-pocket-tts.md`)

| Maszyna | Tryb (affinity 6 rdzeni?) | Model / głos | Zdań | RTF mediana | RTF max | Czas do 1. fragmentu mediana [ms] | p95 [ms] | RAM procesu szczyt [MB] | Uwagi |
|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | |

Cel: RTF ≈ 0,21 (deklarowane), czas do 1. fragmentu ~200 ms; na 6 rdzeniach zapisać rzeczywiste wartości.

## Wnioski do ADR

- ADR (4) ML runtime: 
- ADR (11) silniki głosu v0: 
- ADR (14) lokalny LLM (4B wystarcza? 8B tylko desktop?): 
- Ryzyka zaobserwowane (ErrorDeviceLost, halucynacje na ciszy, throttling laptopa): 
