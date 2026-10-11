# lib-supertonic

Sidecar mowy **Supertonic 3** — osobny proces `alfa-supertonic`, uruchamiany przez `voice-tts-impl` przy pierwszym
zdaniu. ONNX Runtime działa tylko w tym procesie (ADR 0004: w procesie Alfy go nie ma).

## Uruchomienie

```
alfa-supertonic --onnx-dir <katalog modelu> --voices-dir <katalog barw> --ort <onnxruntime.dll>
                [--threads 4] [--steps 6] [--speed 1.05] [--lang pl]
```

W programie: sidecar i `onnxruntime.dll` leżą w `sidecars/supertonic/` (obok programu albo w katalogu danych),
model w `models/supertonic/` (płasko: cztery sieci `.onnx`, `tts.json`, `unicode_indexer.json`, barwy `F1.json`…
`M5.json`). Model pobiera się w Ustawieniach → „Modele lokalne i silniki” (pozycja `supertonic-3`, sumy SHA-256
przypięte w `app-models/src/supertonic.rs`).

## Protokół (linie JSON po stdio)

| Kierunek | Linia |
| --- | --- |
| żądanie | `{"id":1,"op":"synth","text":"…","voice":"F1","rate":1.05,"lang":"pl","steps":6}` — `rate`, `lang`, `steps` opcjonalne |
| odpowiedź | jedna albo więcej `{"type":"audio","id":1,"pcm16":"<base64>","sample_rate":44100}`, potem `{"type":"done","id":1}` |
| błąd | `{"type":"error","id":1,"message":"…"}` |
| inne | `{"id":2,"op":"ping"}` → `done`; `{"id":3,"op":"quit"}` kończy proces |

Linie, które nie są JSON-em, są pomijane po obu stronach. Diagnostyka idzie na stderr.

## Ustawienia przejęte z Delty AI (strojone na sprzęcie)

6 kroków odszumiania, tempo naturalne 1,05, język `pl`, barwy F1–F5 i M1–M5. Agentki: Alfa F1, Beta F2, Gama F4,
Delta F5 (`voice-tts-contract/src/voices.rs`).

## Pomiar (2026-10-11, laptop z RTX 4050, sam procesor)

20 zdań po polsku, 6 kroków: zdanie gotowe po 0,97 s (mediana), 1,42 s (najwolniejsze 5 %), najkrótsze 0,68 s;
czas syntezy to 0,17 czasu trwania dźwięku; start procesu z rozgrzewką 1,1 s. Dla porównania przykład producenta
w Pythonie przy 8 krokach: 2,10 s. Brzmienia nie oceniano automatycznie — odsłuch należy do właściciela.

## Licencje

- Wagi modelu: OpenRAIL-M — wolno używać i rozpowszechniać z przekazaniem listy zakazanych zastosowań
  (opis: `docs/plan/15-glos-lokalny.md`, §5a).
- Przygotowanie tekstu i przebieg syntezy: na podstawie przykładu producenta (`supertone-inc/supertonic`, MIT).
- ONNX Runtime: MIT (biblioteka dołączana jako plik obok sidecara).

## Testy

`cargo test -p lib-supertonic` — protokół na udawanym silniku, przygotowanie tekstu, szum, wymiary barw. Prawdziwy
model sprawdza `alfa-glos-proba/proba_sidecar.py` (poza repozytorium, wymaga pobranego modelu).
