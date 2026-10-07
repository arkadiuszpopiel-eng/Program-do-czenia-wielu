"""Spike (h)/(e): pomiar RTF i czasu do pierwszego fragmentu Pocket TTS dla 20 zdan PL.

Uruchom w venv (patrz bench-pocket-tts.md):
    python bench_pocket_tts.py --ref sciezka\\do\\referencji.wav --out wyniki.csv

Sekcja "DOPASUJ" ponizej zalezy od aktualnego API pakietu pocket-tts (README pakietu).
Skrypt celowo nie ma innych zaleznosci niz numpy i soundfile.
"""

from __future__ import annotations

import argparse
import csv
import os
import statistics
import sys
import time
from pathlib import Path

import numpy as np
import soundfile as sf

# Te same 20 zdan, co w evals/spikes/e-voice-lab/candidates.md (sekcja "Zdania testowe").
SENTENCES = [
    "Dzień dobry, jestem Alfa. W czym mogę dzisiaj pomóc?",
    "Spotkanie z panią Katarzyną Wiśniewską przesunięto na czwartek, 12 marca 2026 roku, na godzinę 14:30.",
    "Plik raport_kwartalny_final_v3.xlsx ma 2,7 megabajta i został zmieniony wczoraj o 23:58.",
    "Zamów dwadzieścia trzy sztuki po 1499 złotych i 99 groszy; razem wychodzi 34 499 złotych i 77 groszy.",
    "Uruchom pull request numer 418 w repozytorium alfa-desktop i sprawdź, czy CI jest zielone.",
    "Temperatura w Zakopanem spadnie w nocy do minus 17 stopni, a w Szczecinie będzie około 3 stopni.",
    "Nie, nie ten plik. Miałam na myśli folder „Zdjęcia z Chorwacji 2024”, nie „Zdjęcia 2025”.",
    "Przełącz na Deltę i poproś ją o podsumowanie ostatnich pięciu wiadomości od Marka Grzegorzewskiego.",
    "Serwer odpowiedział błędem 503 Service Unavailable; spróbuję ponownie za trzydzieści sekund.",
    "W Bydgoszczy, Rzeszowie i Świnoujściu jutro zapowiadają przelotne opady i wiatr do 60 kilometrów na godzinę.",
    "Zainstaluj Node dwadzieścia dwa, potem wpisz pnpm install i uruchom build.",
    "Przypomnij mi o dentyście w poniedziałek o 8:15 rano i o wizycie u Grzegorza we wtorek po południu.",
    "Ostatnia aktualizacja sterownika Adrenalin pochodzi z 3 września; nowsza wersja nie została jeszcze wydana.",
    "Przepraszam, nie dosłyszałam. Czy chodziło o „wyślij”, czy o „wyszlij”?",
    "Mój adres to ulica Świętokrzyska 31/33, mieszkanie 7, 00-049 Warszawa.",
    "Cztery agentki, Alfa, Beta, Gama i Delta, mogą pracować równolegle, ale mówi tylko jedna naraz.",
    "Backup zajął 4 minuty i 12 sekund; skopiowano 18 432 pliki, pominięto 3 z powodu braku uprawnień.",
    "Otwórz w Excelu arkusz „Budżet 2026” i zaznacz komórki od B2 do F14.",
    "Rzeczywisty czas przetwarzania wyniósł zero przecinek dwadzieścia jeden, czyli pięć razy szybciej niż w czasie rzeczywistym.",
    "Dobranoc. Wyłączam mikrofon; jeśli będziesz czegoś potrzebować, naciśnij skrót lub powiedz „Hej Alfa”.",
]


def peak_rss_mb() -> float:
    """Szczytowe zuzycie pamieci procesu w MB (Windows: PeakWorkingSetSize)."""
    try:
        import ctypes
        from ctypes import wintypes

        class PMC(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
            ]

        pmc = PMC()
        pmc.cb = ctypes.sizeof(PMC)
        h = ctypes.windll.kernel32.GetCurrentProcess()
        ctypes.windll.psapi.GetProcessMemoryInfo(h, ctypes.byref(pmc), pmc.cb)
        return pmc.PeakWorkingSetSize / (1024 * 1024)
    except Exception:  # noqa: BLE001 - poza Windows lub brak psapi
        return -1.0


# --- DOPASUJ do README pakietu pocket-tts -------------------------------------------
# Ponizsze nazwy odpowiadaja API znanemu z repozytorium kyutai-labs/pocket-tts (2026):
#   from pocket_tts import TTSModel
#   model = TTSModel.load_model()                       # ewentualnie: load_model("<repo HF modelu PL>")
#   state = model.get_state_for_audio_prompt("ref.wav") # klon glosu z krotkiej referencji
#   audio = model.generate_audio(state, text)           # tensor/ndarray, model.sample_rate
#   for chunk in model.generate_audio_stream(state, text): ...  # strumien (jesli dostepny)
# Jesli nazwy sie roznia, popraw TYLKO ten blok.


def load_model(model_id: str | None):
    from pocket_tts import TTSModel  # type: ignore[import-not-found]

    if model_id:
        return TTSModel.load_model(model_id)
    return TTSModel.load_model()


def make_voice_state(model, ref_path: str | None):
    if ref_path:
        return model.get_state_for_audio_prompt(ref_path)
    return None


def synth_stream(model, state, text: str):
    """Zwraca generator fragmentow audio (ndarray float32) albo None, gdy pakiet nie ma strumienia."""
    fn = getattr(model, "generate_audio_stream", None)
    if fn is None:
        return None
    return fn(state, text) if state is not None else fn(text)


def synth_full(model, state, text: str):
    return model.generate_audio(state, text) if state is not None else model.generate_audio(text)


def to_numpy(x) -> np.ndarray:
    if hasattr(x, "detach"):
        x = x.detach().cpu().numpy()
    return np.asarray(x, dtype=np.float32).reshape(-1)


# --- koniec sekcji DOPASUJ ------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", default=None, help="WAV z referencja glosu (5-10 s) albo brak = glos domyslny")
    ap.add_argument("--model", default=None, help="identyfikator modelu PL (HF) jesli pakiet tego wymaga")
    ap.add_argument("--out", required=True, help="plik CSV z wynikami")
    ap.add_argument("--wav-dir", default="out", help="katalog na wygenerowane WAV")
    ap.add_argument("--warmup", type=int, default=1, help="ile zdan rozgrzewki (nie liczone)")
    args = ap.parse_args()

    t0 = time.perf_counter()
    model = load_model(args.model)
    state = make_voice_state(model, args.ref)
    load_s = time.perf_counter() - t0
    sr = int(getattr(model, "sample_rate", 24000))
    print(f"model zaladowany w {load_s:.2f} s, sample_rate={sr}, watki OMP={os.environ.get('OMP_NUM_THREADS', 'domyslne')}")

    wav_dir = Path(args.wav_dir)
    wav_dir.mkdir(parents=True, exist_ok=True)

    for i in range(args.warmup):
        to_numpy(synth_full(model, state, SENTENCES[i % len(SENTENCES)]))

    rows = []
    for idx, text in enumerate(SENTENCES, start=1):
        first_ms = None
        t_start = time.perf_counter()
        gen = synth_stream(model, state, text)
        if gen is not None:
            chunks = []
            for chunk in gen:
                if first_ms is None:
                    first_ms = (time.perf_counter() - t_start) * 1000.0
                chunks.append(to_numpy(chunk))
            audio = np.concatenate(chunks) if chunks else np.zeros(0, dtype=np.float32)
        else:
            audio = to_numpy(synth_full(model, state, text))
        total_s = time.perf_counter() - t_start
        dur_s = len(audio) / sr if sr else 0.0
        rtf = total_s / dur_s if dur_s > 0 else float("nan")
        sf.write(wav_dir / f"pocket-pl-{idx:02d}.wav", audio, sr)
        rows.append(
            {
                "idx": idx,
                "chars": len(text),
                "audio_s": round(dur_s, 3),
                "total_s": round(total_s, 3),
                "rtf": round(rtf, 3),
                "first_chunk_ms": round(first_ms, 1) if first_ms is not None else "",
            }
        )
        print(f"{idx:02d}: audio {dur_s:5.2f} s, czas {total_s:5.2f} s, RTF {rtf:.3f}, 1. fragment {first_ms if first_ms is None else round(first_ms)} ms")

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    rtfs = [r["rtf"] for r in rows if r["rtf"] == r["rtf"]]  # bez NaN
    firsts = [r["first_chunk_ms"] for r in rows if r["first_chunk_ms"] != ""]

    def p95(v):
        return sorted(v)[max(0, int(round(0.95 * len(v))) - 1)] if v else float("nan")

    rss = peak_rss_mb()
    med_first = statistics.median(firsts) if firsts else "n/d (brak API strumieniowego)"
    p95_first = round(p95(firsts)) if firsts else "n/d"
    print()
    print("Wiersz do results/<maszyna>-<data>.md, sekcja 'Pocket TTS PL':")
    print(
        f"| <maszyna> | <affinity?> | pocket-tts + {args.model or 'model domyslny'} / ref={'wlasny' if args.ref else 'wbudowany'} | {len(rows)} "
        f"| {statistics.median(rtfs):.3f} | {max(rtfs):.3f} | {med_first if isinstance(med_first, str) else round(med_first)} | {p95_first} | {rss:.0f} | zaladowanie modelu {load_s:.1f} s |"
    )
    print(f"CSV: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
