# Spike (e) — Voice Lab PL: kandydaci STT/TTS, ślepa ocena, korpus

**Cel (PLAN §16.2 F0 (e); VOICE.md §15.2; ACCEPTANCE F0-08, F0-09, F0-10):**

1. wypełnić tabelę kandydatów STT/TTS (`candidates.md`): kandydat × PL × licencja × streaming × zasoby × TTFB,
   pomiar na **desktopie i laptopie** (lokalne) lub przez API (chmurowe — tylko jeśli masz klucz; bez klucza wpisz „brak klucza");
2. **ślepa ocena TTS PL 1–5 na 20 zdaniach**, ≥ 3 kandydatów; go = średnia ≥ 4,0 dla najlepszego; no-go = zostają głosy v0
   (nie blokuje F1);
3. **WER PL ≤ 12 %** STT na korpusie własnym — wymaga nagrań z `protokol-nagran.md` (bramka ludzka #3);
4. wynik → ADR (4) ML runtime i ADR (11) silniki głosu v0.

Pliki w tym katalogu:

| Plik | Co |
|---|---|
| `candidates.md` | tabela do wypełnienia + 20 zdań testowych PL + protokół ślepej oceny |
| `protokol-nagran.md` | jak nagrać 12 typów korpusu, format, nazewnictwo, gdzie zapisać (poza gitem) |
| `results/` | `<maszyna>-<data>.md` z wypełnioną tabelą i arkusz ocen |

Kolejność: nagraj korpus (może być równolegle z innymi spike'ami) → zmierz kandydatów lokalnych (desktop, laptop) →
wygeneruj 20 zdań każdym kandydatem TTS → ślepa ocena → WER na korpusie.

## Kandydaci lokalni — jak zmierzyć

| Kandydat | Jak uruchomić | Co wpisać |
|---|---|---|
| whisper.cpp large-v3, large-v3-turbo Q5_0, small Q5 | `h-sprzet/bench-whisper.ps1` z odpowiednim `-Model` (na obu maszynach); WER — patrz niżej | RAM/VRAM, backend, RTF → „opóźnienie finalizacji" ≈ RTF × długość ostatniego segmentu (przyjmij 3 s) |
| Parakeet v3 (ONNX) | `sherpa-onnx` (gotowe binaria Windows w GitHub Releases k2-fsa/sherpa-onnx, model `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`) — `sherpa-onnx-offline.exe` na próbce 60 s | CPU, RAM, czas, WER |
| Pocket TTS PL | `h-sprzet/bench-pocket-tts.md` — daje 20 WAV do oceny + RTF + TTFB | z wiersza wynikowego skryptu |
| Piper pl_PL | GitHub Releases `rhasspy/piper` (Windows zip) + głosy `pl_PL-*` z Hugging Face `rhasspy/piper-voices`; `piper.exe -m glos.onnx -f out.wav` z tekstem na stdin; 20 zdań ze skryptem pętli w PowerShell | RTF (czas / długość), brak streamingu → TTFB = cały czas dla 1. zdania |
| Chatterbox Multilingual / XTTS-v2 / F5-TTS | **tylko laptop (CUDA)**; venv jak w `bench-pocket-tts.md`, pakiet wg README projektu; 20 zdań; mierz czas i VRAM (`nvidia-smi`) | RTF, VRAM; czy mieści się w 6 GB obok whisper turbo (uruchom `whisper-cli` w tle) |
| VibeVoice-Realtime | tylko jeśli ma PL wg README — inaczej „brak PL" | |

**Zasoby** = Private Working Set procesu (Menedżer zadań → Szczegóły → kolumna „Zestaw roboczy (prywatny)")
+ VRAM (jak w `h-sprzet`). **TTFB** = czas do pierwszego fragmentu audio (jeśli brak streamingu: czas całej syntezy
1. zdania). **Streaming** = tak, jeśli silnik zwraca audio fragmentami przed końcem syntezy.

## Kandydaci chmurowi

Tylko z kluczem (bramka #1). Dla każdego: 20 zdań przez API z opcją strumieniową, czas do pierwszego bajta audio
mierzony w skrypcie (sesja AI przygotuje wywołania w worktree spike'u; klucze w zmiennych środowiskowych, nigdy w plikach
repo). Bez klucza — wpisz „brak klucza, odroczone".

## WER PL na korpusie własnym

1. Nagrania typu 1, 2, 5, 6 z `protokol-nagran.md` (razem ~16 min) → transkrypcja referencyjna: **przepisz sam/sama**
   (dokładnie to, co powiedziałeś/aś, z liczbami słownie tak, jak wymówione) do plików `.txt` obok WAV.
2. Transkrypcja kandydatem (`whisper-cli -l pl -otxt` per plik).
3. WER: pakiet `jiwer` (`uv pip install jiwer`) — normalizacja: małe litery, bez interpunkcji. Skrypt liczący
   przygotuje sesja AI (`evals/` w F0 ma szkielet `voice-lab`); ręcznie: `python -c "import jiwer; print(jiwer.wer(open('ref.txt').read(), open('hyp.txt').read()))"`.
4. Wynik per typ nagrania i łącznie; próg ≤ 12 % łącznie (cisza + szum).

## Ślepa ocena TTS (protokół w `candidates.md`)

Skrót: każdy kandydat generuje te same 20 zdań → pliki są **przemieszane i przemianowane** (skrypt losujący w `candidates.md`)
→ słuchasz w losowej kolejności, oceniasz 1–5 w arkuszu → dopiero po ocenie odkrywasz mapę nazw → średnia per kandydat.

## Co skopiować z powrotem

- `results/<maszyna>-<data>.md` = wypełniona kopia `candidates.md` (tabele STT i TTS z wartościami i wersjami),
- `results/ocena-<data>.csv` (plik, kandydat, ocena, uwaga) + `results/mapa-<data>.csv` (odkryta mapa),
- WER per plik jako tabela w `.md`,
- nagrania i WAV kandydatów zostają w `evals/corpus/` (poza gitem).
