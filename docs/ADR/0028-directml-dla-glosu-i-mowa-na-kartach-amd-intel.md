# ADR 0028 — DirectML dla głosu Supertonic i rozpoznawanie mowy na kartach AMD/Intel

| Pole | Wartość |
|---|---|
| Status | **Proponowany** — czeka na podpis właściciela; kod wprowadzony 2026-10-17 na polecenie właściciela („pobierz to, co potrzeba, i dodaj do programu”) |
| Data | 2026-10-17 |
| Powiązane | ADR 0004 (ML bez Pythona — odrzucił DirectML jako wspólny backend), ADR 0014 (llama.cpp), `docs/plan/03-roadmapa.md` C6.6, `docs/modules/voice-tts/SPEC.md`, `docs/modules/device-profile/SPEC.md` |
| Zmienia | ADR 0004 w jednym punkcie: DirectML dopuszczony **opcjonalnie i wąsko** — tylko dla głosu Supertonic w osobnym procesie, z jawnym powrotem na procesor |

## Kontekst

Właściciel: Alfa ma działać na kartach NVIDIA, AMD i Intel oraz na grafice zintegrowanej. Stan przed zmianą:

- **Rozpoznawanie mowy.** ADR 0004 i `PLAN.md` zakładały whisper.cpp z backendem Vulkan dla AMD. Oficjalne wydania
  whisper.cpp **nie mają** kompilacji Vulkan dla Windows — sprawdzone 2026-10-17 listą plików wydań do v1.9.5
  (są: CPU, BLAS, CUDA 11.8/12.4, ARM). Na AMD/Intel Whisper liczył więc na procesorze: na laptopie właściciela
  ok. 54 s na zdanie (Whisper turbo, `app-modules/src/stt.rs`).
- **Głos.** Supertonic 3 i Smart Turn liczą na procesorze przez ONNX Runtime CPU (`ort` z `load-dynamic`).
- **DirectML** (Microsoft) jest w trybie podtrzymania — bez nowych funkcji (`docs/plan/11-technologie-2026-10.md`);
  dlatego ADR 0004 go odrzucił.

## Decyzja

1. **Mowa na AMD/Intel — Qwen3-ASR przez `llama-server` Vulkan** (bez nowego silnika): pakiety 5 i 4 na kartach
   AMD/Intel (także samej zintegrowanej) dostają Qwen3-ASR, Whisper zostaje zapasem. Pomiar 2026-10-17 na
   RX 9070 XT (llama.cpp b11559 Vulkan, te same argumenty co `qwen_wire::build_args`): **0,16–0,23 s na zdanie**
   3–7 s (pierwsze, z rozgrzaniem: 2,2 s), **2878 MB** pamięci karty; 4 zdania z Supertonic (barwa M1): 3 identyczne,
   w jednym zła forma jednego wyrazu („Zapisałem” zamiast „Zapisałam”). Bez osobnej karty (zmierzone liczniki
   systemu: 0 MB wolnej pamięci osobnej karty) Qwen3-ASR liczy na procesorze (ok. 2,4 s na zdanie na laptopie).
   Na karcie z własną pamięcią **poniżej 12 GB** (`app_modules::stt::VULKAN_QWEN_CARD_MIN_MB`) Qwen3-ASR liczy na
   procesorze (przegląd 2026-10-18): ładuje się przed modelem rozmowy i zajmuje ok. 2,9 GB, a model rozmowy zostawia na
   mowę 1,5 GB (rozmiar Whispera turbo) — na karcie 6–11 GB obu by się pewnie nie zmieściło. Rezerwa 3 GB w budżecie
   modelu rozmowy została sprawdzona i odrzucona (po starcie modelu rozmowy wolna pamięć i tak spadała poniżej progu
   3,2 GB Qwen3-ASR). Ograniczenie: reguła działa także wtedy, gdy model rozmowy nie korzysta z karty (klasa
   „nieznana”, rozmowa przez API) — wtedy mowa mogłaby liczyć na karcie; do osobnego budżetu pamięci na kartę (C6.6).
2. **Głos Supertonic na karcie przez DirectML** — opcjonalnie:
   - dwie pozycje katalogu w `sidecars/onnxruntime-dml/`: `onnxruntime-directml` (NuGet
     `Microsoft.ML.OnnxRuntime.DirectML` 1.23.0, MIT) i `directml` (NuGet `Microsoft.AI.DirectML` 1.15.4, licencja
     Microsoftu); pakiety zgodne z `packageHash` (SHA-512) katalogu NuGet, pliki przypięte SHA-256;
   - `alfa-supertonic --dml <numer karty DXGI>`; rejestracja DirectML z `error_on_failure` — nieudana daje komunikat
     na stderr i **jawny** powrót na procesor (sprawdzone: biblioteka bez DirectML, nieistniejąca karta);
   - Alfa przekazuje `--dml` tylko przy zainstalowanych obu pozycjach i karcie z własną pamięcią ≥ 12 GB
     (`GPU_TTS_MIN_VRAM_MB`; osobna przed zintegrowaną z taką rezerwą, potem największa). Pakiety dokładają te pozycje tylko z głosem Supertonic i taką
     kartą. Smart Turn zostaje na procesorze (ok. 25 ms — nie mierzyłam zysku na karcie).
   - Pomiar 2026-10-17 na RX 9070 XT (Ryzen 7 5700X3D), 3 przebiegi × 5 zdań (30,7 s mowy), stan ustalony: procesor
     RTF 0,154–0,159 (średnio 0,95 s na zdanie), DirectML RTF 0,094–0,108 (0,58 s na zdanie, prawie niezależnie od
     długości; najdłuższe zdanie 1,57 s → 0,58 s); start procesu 3,4 s zamiast 1,1 s; **538 MB** pamięci karty.
     Próg 12 GB: na karcie 8 GB model 4.5B i STT zajmują już 7380 z 7408 MB budżetu.
   - `DirectML.dll` ładuje się z katalogu `onnxruntime.dll` (lista modułów procesu), nie z `System32`.

## Konsekwencje

- Na wolnej karcie AMD od 12 GB (≥ 3,2 GB wolnej pamięci) mowa jest rozpoznawana na karcie — zmierzone na RX 9070 XT, bez
  kompilowania whisper.cpp i bez binariów spoza oficjalnych wydań. Gdy model rozmowy zajmie kartę, Qwen3-ASR liczy na
  procesorze (ok. 2,4 s na zdanie), a NVIDIA ma wtedy jeszcze Whisper na karcie. Karty Intel — niesprawdzone.
  Na karcie AMD/Intel poniżej 12 GB mowa liczy na procesorze (ok. 2,4 s na zdanie — ponad 20 razy szybciej niż
  Whisper na procesorze w 0.0.62). Na NVIDIA ≥ 8 GB w pakiecie „Wzorcowy” (Qwen3-ASR przez CUDA) obok modelu rozmowy
  zostaje rezerwa 1,5 GB — stan sprzed 0.0.63, do rozwiązania osobnym budżetem pamięci na kartę (C6.6).
- Licencja DirectML pozwala rozpowszechniać bibliotekę w aplikacjach (§1a), ale **dopuszcza zbieranie danych
  diagnostycznych przez Microsoft (§2a)**. Pozycja `directml` ma `confirmed: false` (znaczek „Do potwierdzenia przez
  człowieka”) i jest pobierana przez użytkownika z NuGet, nie dołączana do instalatora.
- Ryzyko: DirectML w trybie podtrzymania; następcą jest Windows ML. Przy kolejnej wersji ONNX Runtime trzeba
  sprawdzić, czy wariant DirectML nadal jest wydawany — inaczej głos wraca na procesor (bez utraty funkcji).
- Numer karty dla DirectML to kolejność adapterów DXGI w procesie Alfy przy składaniu głosu; `alfa-supertonic`
  wylicza ją po swojemu przy starcie. Zwykle są takie same — różnić je może preferencja karty ustawiona w Windows
  osobno dla `alfa-supertonic.exe` (Ustawienia → Ekran → Grafika); wtedy głos liczy na innej karcie, a przy
  nieistniejącym numerze — na procesorze. Niesprawdzone na maszynie z kilkoma kartami.
- Pamięć karty: Supertonic (538 MB) nie ma dzierżawy w `model-residency` — dlatego próg 12 GB. Na kartach NVIDIA i
  Intel ≥ 12 GB DirectML włącza się tak samo, ale nie był mierzony (tylko AMD).
- Karta zintegrowana z rezerwą z BIOS-u ≥ 6 GB liczy się jak karta z tą pamięcią (`GpuInfo::has_memory_pool`), więc
  duże APU (np. Ryzen AI Max) też mogą dostać DirectML; niezmierzone.

## Do decyzji właściciela

1. Podpis: DirectML opcjonalnie dla Supertonic (jak wyżej) — tak / nie (wtedy pozycje i flagę `--dml` usuwamy).
2. Potwierdzenie licencji DirectML z klauzulą danych diagnostycznych (pozycja `directml`).
3. Czy mierzyć i włączać DirectML także na karcie zintegrowanej obok osobnej (laptop) — dziś wyłączone, bo
   niezmierzone.
