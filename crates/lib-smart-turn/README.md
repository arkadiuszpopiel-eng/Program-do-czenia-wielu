# lib-smart-turn

Sidecar końca wypowiedzi **Smart Turn v3.2** — osobny proces `alfa-smart-turn`, uruchamiany przez `voice-turn-impl`
na początku rozmowy głosowej. Model słucha ostatnich 8 s nagrania i odpowiada jednym prawdopodobieństwem: czy
rozmówca skończył mówić. ONNX Runtime działa tylko w tym procesie (ADR 0004: w procesie Alfy go nie ma).

## Uruchomienie

```
alfa-smart-turn --model <smart-turn-v3.2-cpu.onnx> --ort <onnxruntime.dll> [--threads 4]
```

W programie: sidecar leży w `sidecars/smart-turn/` (obok programu albo w katalogu danych), model w `models/turn/`,
biblioteka ONNX Runtime obok sidecara albo w `sidecars/onnxruntime/` (pozycje katalogu `smart-turn-v3.2`
i `onnxruntime-cpu` w `app-models/src/voice_runtime.rs`, sumy SHA-256 przypięte). Sidecary do zbudowanego programu
kładzie `scripts/stage-sidecars.sh`.

## Protokół (linie JSON po stdio)

| Kierunek | Linia |
| --- | --- |
| żądanie | `{"id":1,"op":"score","pcm16":"<base64>"}` — nagranie 16 kHz mono, PCM16 LE; liczy się ostatnie 8 s |
| odpowiedź | `{"type":"score","id":1,"probability":0.95,"ms":24}` |
| błąd | `{"type":"error","id":1,"message":"…"}` |
| inne | `{"id":2,"op":"ping"}` → `{"type":"done","id":2}`; `{"id":3,"op":"quit"}` kończy proces |

Linie, które nie są JSON-em, są pomijane. Diagnostyka idzie na stderr.

## Cechy dźwięku (`src/mel.rs`)

Tak jak we wzorcowym środowisku wydawcy modelu: ostatnie 8 s (krótsze nagranie dopełnione zerami z lewej),
normalizacja do średniej 0 i odchylenia 1, okno Hanna 400 próbek, krok 160, dopełnienie odbiciem, widmo mocy,
80 pasm Slaneya, logarytm dziesiętny, odrzucenie ostatniej ramki, obcięcie do „maksimum − 8”, potem (x + 4) / 4.
Wynik: tablica 1 × 80 × 800.

## Dlaczego osobny proces, a nie `tract`

Model jest skwantowany do liczb 8-bitowych. Biblioteka `tract` (czysty Rust, używana dla Silero VAD) wczytuje go,
ale liczy inaczej niż ONNX Runtime: dla tego samego nagrania 0,33 zamiast 0,95 (pomiar 2026-10-11, cechy wejściowe
zgodne co do 0,000005). Dlatego model liczy ONNX Runtime w procesie pobocznym — tym samym plikiem biblioteki co głos
Supertonic.

## Pomiar (2026-10-11, laptop z RTX 4050, sam procesor)

| Co | Wynik |
| --- | --- |
| pierwsza ocena po starcie procesu (z wczytaniem modelu) | 0,21–0,25 s |
| kolejne oceny | 25–46 ms |
| zgodność z ONNX Runtime w Pythonie (trzy przycięcia tego samego zdania) | 0,9523 / 0,0184 / 0,1502 — identycznie do czwartego miejsca |

Przycięte w połowie zdania nagranie dostaje 0,02–0,15 („jeszcze nie skończył”), całe zdanie 0,95 („skończył”).

## Licencje

- Model: BSD-2-Clause (`pipecat-ai/smart-turn-v3`).
- ONNX Runtime: MIT (biblioteka dołączana jako plik, nie wkompilowana).

## Testy

`cargo test -p lib-smart-turn` — protokół na udawanym modelu i cechy dźwięku. Prawdziwy model:
`cargo test -p voice-turn-impl --test smart_turn_live -- --nocapture` ze zmiennymi `ALFA_SMART_TURN_EXE`,
`ALFA_SMART_TURN_MODEL`, `ALFA_SMART_TURN_ORT`, `ALFA_SMART_TURN_WAV` (opcjonalnie `ALFA_SMART_TURN_REF` — wzorzec
z `alfa-glos-proba/proba_turn.py`).
