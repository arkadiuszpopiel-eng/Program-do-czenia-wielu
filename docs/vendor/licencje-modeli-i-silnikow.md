# Licencje modeli i silników — sprawdzone u źródła (C5.1)

Stan na **2026-10-21**. Badanie: 5 agentów (strony wydawców, pliki LICENSE w repozytoriach autorów, karty modeli
Hugging Face, NuGet) + krytyk, który ponownie otworzył najbardziej wątpliwe źródła. Porównanie z kartami licencji w programie
(`crates/app-models/data/license-cards.json`, 24 pozycje). **To nie jest opinia prawna** — pole `confirmed` w kartach zostaje
`false`, dopóki właściciel nie potwierdzi pozycji. Cytaty ≤ 15 słów; niczego nie pobierano poza stronami i plikami tekstowymi.

## Najważniejsze dla właściciela

1. **Bielik (4.5B i 1.5B)** — poza Apache-2.0 wydawca stosuje _Regulamin bielik.ai_ (v1.1 z 31.07.2025): m.in. zakaz
   wykorzystywania Modelu do przetwarzania danych osobowych i wiek 18+. Dla asystenta z pamięcią rozmów to sprawa do decyzji.
   Bielik 4.5B wywodzi się z Qwen2.5-3B, który Alibaba wydała na _Qwen Research License_ (użycie komercyjne wymaga licencji od
   Alibaba Cloud); SpeakLeash przypisuje Qwen2.5 licencję Apache-2.0 — źródła są niespójne. Bielik 1.5B (z Qwen2.5-1.5B) — Apache-2.0.
2. **DirectML** (`Microsoft.AI.DirectML`) ma własną licencję Microsoftu (MICROSOFT SOFTWARE LICENSE TERMS), nie MIT — warunki
   redystrybucji obok programu.
3. **Supertonic 3** — wagi na BigScience Open RAIL-M (ograniczenia użycia); licencja wymaga przekazania jej kopii — **naprawione**:
   katalog instaluje `LICENSE` (15 007 B, z tej samej rewizji, z sumą SHA-256) obok modelu przy nowej instalacji.
4. **ONNX Runtime — telemetria:** biblioteka `ort` domyślnie włączała telemetrię oficjalnych buildów Microsoftu w procesach
   pobocznych mowy — **naprawione** (`with_telemetry(false)` w `lib-supertonic` i `lib-smart-turn`).
5. **Bielik — integralność pobrania:** manifest miał pusty `sha256` i adres `/resolve/main` — **naprawione**: hash z API Hugging Face
   (LFS oid) i adres przypięty do rewizji (`providers-local-impl/models.toml`).
6. **„Pocket TTS PL”** — katalog Alfy nie podaje adresu pobrania polskich wag; jedyny znaleziony kandydat to model społeczności
   `shefowl/pocket-tts-polish-6l` (CC-BY-4.0, nieoficjalny — Kyutai tylko go wymienia). Oficjalne wagi `kyutai/pocket-tts`: CC-BY-4.0
   z dostępem po akceptacji warunków (zakaz klonowania głosu bez zgody i in.). Pocket TTS nie dodaje znaku wodnego.
7. **FFmpeg** — Alfa go nie pobiera (instalacja ręczna); licencja zależy od kompilacji: dokumenty repozytorium zalecają
   `winget install Gyan.FFmpeg`, a kompilacje gyan.dev są na GPL-3.0 — karta podaje „LGPL-2.1+ albo GPL-2.0+”.
8. **WeSpeaker (VoxCeleb)** i **openWakeWord (modele)** — warunki użycia komercyjnego nieustalone u źródła (dane badawcze, pytania
   do autora bez odpowiedzi).

## Zestawienie

| Pozycja                                 | Licencja u źródła                                                                           | Zgodna z kartą | Komercyjnie | Wskazanie autora | Pewność |
| --------------------------------------- | ------------------------------------------------------------------------------------------- | -------------- | ----------- | ---------------- | ------- |
| `bielik-4.5b-v3.0-instruct-q8_0`        | Apache-2.0 według metadanych i karty modelu (GGUF, Instruct i bazowy Bielik-4.5B-v3). …     | **nie**        | warunkowo   | tak              | średnia |
| `bielik-1.5b-v3.0-instruct-q8_0`        | Apache-2.0 według metadanych i karty modelu (GGUF, Instruct i bazowy Bielik-1.5B-v3). Do …  | **nie**        | tak         | tak              | wysoka  |
| `sidecar-llama-vulkan`                  | MIT (LICENSE przy tagu b11559, informacja o prawach autorskich: The ggml authors, …         | tak            | tak         | tak              | wysoka  |
| `sidecar-llama-cuda`                    | llama.cpp: MIT, z tymi samymi komponentami CPU co pozostałe paczki (LLVM OpenMP, …          | tak            | warunkowo   | tak              | średnia |
| `sidecar-llama-cpu`                     | MIT (LICENSE przy tagu b11559, prawa autorskie: The ggml authors, 2023–2026). Komponenty …  | tak            | tak         | tak              | wysoka  |
| `multilingual-e5-small`                 | MIT (tylko jako znacznik metadanych karty modelu: license: mit; w repozytorium HF nie ma …  | tak            | tak         | tak              | wysoka  |
| `paraphrase-multilingual-minilm-l12-v2` | Apache-2.0 (znacznik metadanych karty: license: apache-2.0; w repozytorium HF nie ma …      | tak            | tak         | tak              | wysoka  |
| `whisper-large-v3-turbo-q5_0`           | Wagi OpenAI: MIT (README i LICENSE w github.com/openai/whisper; karta HF …                  | tak            | tak         | tak              | wysoka  |
| `whisper-small-q5_1`                    | Wagi OpenAI z repozytorium GitHub: MIT. Konwersja ggml: MIT (znacznik metadanych HF …       | tak            | tak         | tak              | wysoka  |
| `qwen3-asr-1.7b-q8`                     | Wydawca GGUF (ggml-org) nie podaje licencji: karta nie ma pola license, w tagach nie ma …   | tak            | tak         | tak              | wysoka  |
| `smart-turn-v3.2`                       | BSD-2-Clause: tag license:bsd-2-clause na HF oraz plik LICENSE w repozytorium GitHub …      | tak            | tak         | tak              | wysoka  |
| `onnxruntime-cpu`                       | MIT („Copyright (c) Microsoft Corporation”)                                                 | tak            | tak         | tak              | wysoka  |
| `onnxruntime-directml`                  | MIT. Strona pakietu nie pokazuje wyrażenia SPDX, tylko link „License Info”; plik licencji … | tak            | tak         | tak              | wysoka  |
| `directml`                              | Własna licencja Microsoftu, nie MIT: „MICROSOFT SOFTWARE LICENSE TERMS — MICROSOFT …        | tak            | warunkowo   | nie              | wysoka  |
| `supertonic-3`                          | Wagi ONNX, konfiguracja i barwy F1–F5/M1–M5: BigScience Open RAIL-M z 18.08.2022 (w …       | tak            | warunkowo   | tak              | wysoka  |
| `piper-pl_PL-gosia-medium`              | Repozytorium piper-voices: MIT tylko w metadanych HF (w drzewie v1.0.0 nie ma pliku …       | **nie**        | nieustalone | nieustalone      | średnia |
| `sidecar-piper`                         | Piper: MIT (LICENSE.md, Copyright (c) 2022 Michael Hansen). Według reguł instalacji w …     | tak            | warunkowo   | tak              | wysoka  |
| `sidecar-pocket-tts`                    | Kod Pocket TTS (Kyutai): MIT. Wagi kyutai/pocket-tts: CC-BY-4.0, dostęp po akceptacji …     | tak            | nieustalone | tak              | średnia |
| `silero-vad`                            | MIT (plik LICENSE w repozytorium autora; w metadanych PyPI pole license jest puste, jest …  | tak            | tak         | tak              | wysoka  |
| `openwakeword-features`                 | Kod: Apache-2.0 (LICENSE i klasyfikator PyPI). Modele: README (to samo brzmienie w 0.5.1 …  | tak            | nieustalone | tak              | średnia |
| `wespeaker-resnet34`                    | Kod WeSpeaker: Apache-2.0. Kod sherpa-onnx: Apache-2.0. MODEL: dokumentacja WeSpeaker …     | **nie**        | nieustalone | tak              | średnia |
| `sidecar-whisper-cpu`                   | MIT (whisper.cpp i ggml, „The ggml authors”). Archiwum zawiera też SDL2.dll (SDL 2.28.5) …  | tak            | tak         | tak              | wysoka  |
| `sidecar-whisper-cuda`                  | MIT (whisper.cpp/ggml) dla programu, licencja NVIDIA CUDA Toolkit EULA (wersja 12.4) dla …  | tak            | warunkowo   | tak              | średnia |
| `sidecar-ffmpeg`                        | Kod FFmpeg: domyślnie LGPL-2.1-or-later. Z --enable-gpl: GPL-2.0-or-later. Z …              | **nie**        | warunkowo   | tak              | wysoka  |

## Szczegóły

### `bielik-4.5b-v3.0-instruct-q8_0`

- **Karta w programie:** Apache-2.0 (Apache License 2.0), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://huggingface.co/speakleash/Bielik-4.5B-v3.0-Instruct-GGUF (plik Bielik-4.5B-v3.0-Instruct.Q8_0.gguf pobierany przez /resolve/main; stan repozytorium: commit 3ee7b47bcd0ad7bc6a4c45bb7bad3e0b2fe066ad, lastModified 2026-01-28; rozmiar 5 061 215 424 B, SHA-256 z API HF (LFS oid) 562f2291de257890adf2b4a914da8b194affe6a7a838a6b7ef3d342f306c1b7f)
- **Licencja u źródła:** Apache-2.0 według metadanych i karty modelu (GGUF, Instruct i bazowy Bielik-4.5B-v3). Dodatkowo wydawca publikuje Regulamin bielik.ai (wersja 1.1 z 31.07.2025), który według swojej treści obowiązuje każdego, kto korzysta z Modelu (Bielik 3, wersja bazowa i dostrojona, oraz modele wcześniejsze). Model bazowy Qwen2.5-3B: Qwen RESEARCH LICENSE AGREEMENT z 19.09.2024 (na HF: license: other, license_name: qwen-research).
- **Tekst licencji:** https://huggingface.co/speakleash/Bielik-4.5B-v3.0-Instruct-GGUF/blob/main/README.md (licencja podana tylko w metadanych i opisie, w repozytorium GGUF nie ma pliku LICENSE); regulamin: https://bielik.ai/regulamin; licencja modelu bazowego: https://huggingface.co/Qwen/Qwen2.5-3B/blob/main/LICENSE
- **Wydawca:** SpeakLeash i ACK Cyfronet AGH (kwantyzację GGUF przygotował Remigiusz Kinas z SpeakLeash). Regulamin wskazuje jako podmiot Fundację SpeakLeash z siedzibą we Wrocławiu, KRS 0001099568. Model bazowy Qwen2.5-3B: Alibaba Cloud.
- **Ograniczenia i warunki:**
  - Regulamin bielik.ai v1.1: korzystanie z Modelu oznacza akceptację regulaminu, a jego nieakceptowanie wydawca traktuje jak brak akceptacji licencji Apache 2.0 (preambuła).
  - Regulamin, sekcja „Korzystanie z usług i Modelu”: zakazane jest m.in. wykorzystywanie usług i Modelu do przetwarzania jakichkolwiek danych osobowych, tworzenie treści szkodliwych lub dyskryminujących, naruszanie cudzej własności intelektualnej i obchodzenie zabezpieczeń Modelu promptami. Zakaz dotyczący danych osobowych może mieć znaczenie dla asystenta osobistego z pamięcią rozmów. Oceny prawnej nie przeprowadzono.
  - Regulamin: z Modelu mogą korzystać osoby, które ukończyły 18 lat; nazwy i logo wolno używać tylko w niezmienionej formie, żeby wskazać, że korzysta się z Modelu; użytkownik zwalnia Fundację z odpowiedzialności wobec osób trzecich. O użyciu komercyjnym regulamin nie mówi wprost i w tej kwestii odsyła do Apache 2.0.
  - Model bazowy Qwen2.5-3B (Qwen Research License): §2(a) daje prawo używania, kopiowania i modyfikowania tylko w celach niekomercyjnych (definicja w §1(i): badania lub ewaluacja). §2(b): przy użyciu komercyjnym trzeba wystąpić do Alibaba Cloud o licencję.
  - Qwen Research License, §3 (rozpowszechnianie Materials lub utworów pochodnych): każdy odbiorca dostaje kopię umowy, zmienione pliki są wyraźnie oznaczone, a w pliku Notice znajduje się informacja o licencji Qwen i prawach Alibaba Cloud. §3(d) pozwala dodać własne warunki do własnych modyfikacji, o ile przestrzega się umowy. §4(b): kto tworzy, trenuje lub dostraja model AI przy użyciu Materials i go udostępnia, ma w dokumentacji wyraźnie pokazać „Built with Qwen” albo „Improved using Qwen”. §5(a): utwory pochodne należą do ich twórcy, pod warunkiem zgodności z umową. §7(b): przy naruszeniu umowę można wypowiedzieć.
  - Czy warunki Qwen przechodzą na Bielika 4.5B, źródła mówią różnie. Karta Bielik-4.5B-v3 podaje, że model zainicjowano z Qwen2.5 3B, a raport techniczny (arXiv 2505.02550, §2.1) opisuje adaptację modeli Qwen2.5 1.5B i 3B (Depth Up-Scaling, wymiana tokenizera na APT4). Ten sam raport uzasadnia wybór Qwen2.5 licencją Apache 2.0. Tymczasem blog Qwen2.5 wyłącza wersje 3B i 72B z Apache 2.0, a repozytorium Qwen2.5-3B ma licencję qwen-research. SpeakLeash na kartach w ogóle nie wspomina licencji Qwen. Rozstrzygnięcie prawne nie należało do zadania.
  - Pierwotne repozytorium speakleash/Bielik-4.5B-v3.0-Instruct ma dostęp warunkowy (gated: auto, wymaga udostępnienia danych konta HF). Repozytorium GGUF, z którego pobiera program, nie ma takiego warunku (gated: false).
  - Karta ostrzega, że model nie ma mechanizmów moderacji, może podawać nieprawdę lub treści obraźliwe, a wersje kwantyzowane odpowiadają gorzej i częściej halucynują. Karta prosi też o cytowanie raportu technicznego i karty modelu.
  - Dane treningowe: według abstraktu raportu to 292 mld tokenów z 303 mln dokumentów (polskie korpusy). W sprawdzonych źródłach nie znaleziono licencji tych danych, więc tej kwestii nie ustalono.
- **Znak wodny:** Nie dotyczy, to model tekstowy, nie silnik mowy. Sprawdzone źródła nie wspominają o znakowaniu generowanych treści.
- **Rozbieżności z kartą:**
  - Identyfikator licencji się zgadza (Apache-2.0), ale karta pomija Regulamin bielik.ai v1.1. Wydawca wiąże go z korzystaniem z Modelu (zakaz przetwarzania danych osobowych, wiek 18+, zasady używania nazwy i logo, zwolnienie z odpowiedzialności).
  - Karta podaje „SpeakLeash (Fundacja Spichlerz)”, a regulamin wydawcy nazywa podmiot „Fundacja SpeakLeash” (KRS 0001099568).
  - Wewnętrzna niespójność u źródła: karta GGUF podaje „Finetuned from: Bielik-4.5B-v2”, a karta Instruct podaje, że model dostrojono z Bielik-4.5B-v3.
  - Raport techniczny SpeakLeash przypisuje modelom Qwen2.5 licencję Apache 2.0, ale Qwen2.5-3B ma u Alibaby licencję qwen-research (Qwen Research License). Karta słusznie zostawia tę kwestię człowiekowi.
  - W repozytorium GGUF nie ma pliku LICENSE ani NOTICE, licencja jest zadeklarowana tylko w metadanych. Karta odsyła do ogólnego tekstu na apache.org.
  - Poza licencją: providers-local-impl/models.toml ma sha256 = "" i adres /resolve/main bez przypiętej rewizji, choć HF podaje SHA-256 pliku (562f2291…1b7f). Rozmiar 5 061 215 424 B zgadza się z komentarzem w manifeście.
- **Dowody:**
  - https://huggingface.co/speakleash/Bielik-4.5B-v3.0-Instruct-GGUF/raw/main/README.md — „license: apache-2.0”
  - https://huggingface.co/speakleash/Bielik-4.5B-v3.0-Instruct-GGUF/raw/main/README.md — „Finetuned from: Bielik-4.5B-v2”
  - https://huggingface.co/speakleash/Bielik-4.5B-v3 — „Initialized from: Qwen2.5 3B”
  - https://huggingface.co/api/models/speakleash/Bielik-4.5B-v3.0-Instruct-GGUF — „gated: false; cardData.license: apache-2.0; sha: 3ee7b47bcd0ad7bc6a4c45bb7bad3e0b2fe066ad”
  - https://huggingface.co/api/models/speakleash/Bielik-4.5B-v3.0-Instruct — „gated: auto; license: apache-2.0”
  - https://huggingface.co/api/models/Qwen/Qwen2.5-3B — „license: other; license_name: qwen-research”
  - https://huggingface.co/Qwen/Qwen2.5-3B/blob/main/LICENSE — „If you are commercially using the Materials, you shall request a license from us.”
  - https://qwenlm.github.io/blog/qwen2.5/ — „Blog Qwen2.5 wyłącza warianty 3B i 72B z licencji Apache 2.0 (parafraza).”
  - https://arxiv.org/html/2505.02550 — „§2.1: Bielik v3 zaadaptowano z Qwen2.5 1.5B i 3B, wybór uzasadniono m.in. licencją Apache 2.0 (parafraza).”
  - https://bielik.ai/regulamin — „wersja 1.1 z dnia 31.07.2025; Fundacja SpeakLeash, KRS: 0001099568”

### `bielik-1.5b-v3.0-instruct-q8_0`

- **Karta w programie:** Apache-2.0 (Apache License 2.0), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/speakleash/Bielik-1.5B-v3.0-Instruct-GGUF (plik Bielik-1.5B-v3.0-Instruct.Q8_0.gguf pobierany przez /resolve/main; stan repozytorium: commit 99d72cc757be4f8b958018f12d6898bad5c19c9e, lastModified 2026-01-28; rozmiar 1 699 568 288 B, SHA-256 z API HF (LFS oid) e206ebbd9ee00cc8f14af5a096ad3f193a59608c63905316e8dc680c96655a60)
- **Licencja u źródła:** Apache-2.0 według metadanych i karty modelu (GGUF, Instruct i bazowy Bielik-1.5B-v3). Do tego Regulamin bielik.ai (wersja 1.1 z 31.07.2025), który wydawca stosuje do Modelu Bielik 3. Model bazowy Qwen2.5-1.5B: Apache-2.0 (plik LICENSE w repozytorium Qwen, z informacją o prawach autorskich Alibaba Cloud z 2024 r.).
- **Tekst licencji:** https://huggingface.co/speakleash/Bielik-1.5B-v3.0-Instruct-GGUF/blob/main/README.md (licencja tylko w metadanych i opisie, w repozytorium GGUF nie ma pliku LICENSE); regulamin: https://bielik.ai/regulamin; licencja modelu bazowego: https://huggingface.co/Qwen/Qwen2.5-1.5B/blob/main/LICENSE
- **Wydawca:** SpeakLeash i ACK Cyfronet AGH. Regulamin wskazuje jako podmiot Fundację SpeakLeash, KRS 0001099568. Model bazowy Qwen2.5-1.5B: Alibaba Cloud.
- **Ograniczenia i warunki:**
  - Apache-2.0 (Bielik i Qwen2.5-1.5B): przy rozpowszechnianiu trzeba dołączyć kopię licencji, zachować informacje o prawach autorskich i oznaczyć zmienione pliki.
  - Regulamin bielik.ai v1.1 (dotyczy Modelu Bielik 3, wersji bazowej i dostrojonej): korzystanie oznacza akceptację. Zakazane jest m.in. wykorzystywanie Modelu do przetwarzania jakichkolwiek danych osobowych, tworzenie treści szkodliwych lub dyskryminujących i obchodzenie zabezpieczeń. Dotyczy osób, które ukończyły 18 lat. Nazwy i logo wolno używać tylko w niezmienionej formie, a użytkownik zwalnia Fundację z odpowiedzialności. O użyciu komercyjnym regulamin nie mówi wprost i odsyła do Apache 2.0.
  - Pierwotne repozytorium speakleash/Bielik-1.5B-v3.0-Instruct wymaga zgody na udostępnienie danych konta HF. Repozytorium GGUF, z którego pobiera program, ma gated: false.
  - Karta prosi o cytowanie raportu technicznego (arXiv 2505.02550) i karty modelu, a także ostrzega, że model nie ma moderacji.
  - Licencja Qwen2.5-1.5B to Apache 2.0, więc problem z licencją badawczą Qwen dotyczy tylko wariantu 4.5B (z bazy Qwen2.5-3B), nie tego modelu.
  - Dane treningowe: licencji korpusów nie znaleziono w sprawdzonych źródłach, tej kwestii nie ustalono.
- **Znak wodny:** Nie dotyczy, to model tekstowy. Sprawdzone źródła nie wspominają o znakowaniu treści.
- **Rozbieżności z kartą:**
  - Licencja Apache-2.0 modelu i bazy Qwen2.5-1.5B potwierdzona u źródła (HF), więc zastrzeżenie „według manifestu” można zamknąć. Karta pomija jednak Regulamin bielik.ai v1.1, który wydawca wiąże z korzystaniem z Modelu (m.in. zakaz przetwarzania danych osobowych, wiek 18+).
  - Karta podaje „Fundacja Spichlerz”, a regulamin wydawcy „Fundacja SpeakLeash” (KRS 0001099568).
  - Niespójność u źródła: karta GGUF podaje „Finetuned from: Bielik-1.5B-v2”, a karta Instruct podaje Bielik-1.5B-v3.
  - Karta pisze „suma pliku jeszcze nieprzypięta”, a HF podaje SHA-256 e206ebbd…5a60 i rozmiar 1 699 568 288 B (zgodny z komentarzem w models.toml). Adres /resolve/main nie przypina rewizji.
  - W repozytorium GGUF nie ma pliku LICENSE ani NOTICE.
- **Dowody:**
  - https://huggingface.co/speakleash/Bielik-1.5B-v3.0-Instruct-GGUF/raw/main/README.md — „license: apache-2.0”
  - https://huggingface.co/speakleash/Bielik-1.5B-v3.0-Instruct-GGUF/raw/main/README.md — „Finetuned from: Bielik-1.5B-v2”
  - https://huggingface.co/speakleash/Bielik-1.5B-v3 — „Initialized from: Qwen2.5 1.5B”
  - https://huggingface.co/api/models/speakleash/Bielik-1.5B-v3.0-Instruct-GGUF — „gated: false; cardData.license: apache-2.0; sha: 99d72cc757be4f8b958018f12d6898bad5c19c9e”
  - https://huggingface.co/Qwen/Qwen2.5-1.5B/blob/main/LICENSE — „Apache License, Version 2.0, January 2004; Copyright 2024 Alibaba Cloud”
  - https://bielik.ai/regulamin — „wersja 1.1 z dnia 31.07.2025”
- **Uwaga krytyka:** Wewnętrzna niespójność. Wynik podaje commercial_use=tak, a jednocześnie opisuje wiążący Regulamin bielik.ai v1.1 z zakazem wykorzystywania Modelu do przetwarzania jakichkolwiek danych osobowych (sprawdzone na bielik.ai/regulamin: wersja 1.1, 31.07.2025, KRS 0001099568). Dla 4.5B ten sam regulamin jest częścią uzasadnienia oceny warunkowo. matches_card=false wynika tylko z pominięcia regulaminu, choć identyfikator i pole commercial zgadzają się z kartą. W innych wpisach (np. onnxruntime-cpu, sidecar-whisper-cuda) podobne pominięcia nie zmieniają matches_card na false.

### `sidecar-llama-vulkan`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://github.com/ggml-org/llama.cpp/releases/tag/b11559 (plik llama-b11559-bin-win-vulkan-x64.zip, 32 084 119 B, SHA-256 bf2fb2a7fc5d7669ccabbb6de3f6dd6b54058f213b32e731abc62b590f8566dc według GitHuba, zgodne z data.rs; wydanie z 2026-10-11T19:14:38Z)
- **Licencja u źródła:** MIT (LICENSE przy tagu b11559, informacja o prawach autorskich: The ggml authors, 2023–2026). Komponenty wbudowane według skryptów budowania tego tagu: LLVM OpenMP 20.1.8 (libomp.dll z plikiem LICENSE-LLVM-OpenMP) na licencji Apache-2.0 WITH LLVM-exception (część kodu na dawnej licencji NCSA/MIT), BoringSSL 0.20260929.0 (Apache-2.0), cpp-httplib (MIT, yhirose) i nlohmann/json (MIT, Niels Lohmann).
- **Tekst licencji:** https://github.com/ggml-org/llama.cpp/blob/b11559/LICENSE
- **Wydawca:** ggml-org (prawa autorskie: The ggml authors)
- **Ograniczenia i warunki:**
  - MIT wymaga dołączenia informacji o prawach autorskich i treści zezwolenia do kopii programu.
  - Skład archiwum według .github/workflows/release.yml i release-publish.yml przy b11559: zadanie Vulkan pakuje tylko ggml-vulkan.dll, a krok publikacji dokłada do niego całą zawartość archiwum windows-cpu (llama-server.exe, biblioteki ggml, libomp.dll i LICENSE-LLVM-OpenMP).
  - Biblioteka uruchomieniowa OpenMP pochodzi z oficjalnego wydania LLVM (GGML_OPENMP_FETCH=ON, wersja 20.1.8, plik licencji kopiowany obok DLL), licencja Apache-2.0 WITH LLVM-exception.
  - BoringSSL jest wbudowany (LLAMA_BUILD_BORINGSSL=ON), a jego licencję rejestruje license_add_file. Od PR #18696 (scalony 10.01.2026) licencje są osadzane w plikach wykonywalnych i program wyświetla je opcją --license. Działania tej opcji w b11559 nie sprawdzano, bo binariów nie pobierano.
  - Skrypty publikacji nie kopiują głównego pliku LICENSE do archiwów Windows. Dokumentacja Alfy (docs/modules/models/SPEC.md, Fala 6) odnotowała pliki LICENSE-* w archiwach b6710. Zawartości archiwum b11559 nie sprawdzano.
  - Archiwum nie zawiera ładowarki Vulkan (vulkan-1.dll) ani sterownika, oba pochodzą ze sterownika karty i nie są częścią paczki (według skryptu budowania).
- **Znak wodny:** Nie dotyczy, to serwer wnioskowania, nie silnik mowy.
- **Rozbieżności z kartą:**
  - Karta odsyła do LICENSE na gałęzi master, a nie przy tagu b11559, z którego pobiera program. Treść przy tagu to też MIT.
  - Karta podaje autora „Georgi Gerganov i współtwórcy”, a informacja o prawach autorskich w LICENSE brzmi „The ggml authors” (różnica drobna).
  - Karta nie wymienia licencji komponentów wbudowanych w paczkę: LLVM OpenMP (Apache-2.0 WITH LLVM-exception), BoringSSL (Apache-2.0), cpp-httplib (MIT) i nlohmann/json (MIT).
- **Dowody:**
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/LICENSE — „MIT License; Copyright (c) 2023-2026 The ggml authors”
  - https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11559 — „llama-b11559-bin-win-vulkan-x64.zip 32084119 bf2fb2a7fc5d7669ccabbb6de3f6dd6b54058f213b32e731abc62b590f8566dc”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/.github/workflows/release-publish.yml — „for target_zip in artifact/llama-bin-win-*-${arch}.zip”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/ggml/src/CMakeLists.txt — „set(GGML_OPENMP_LLVM_VERSION "20.1.8")”
  - https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-20.1.8/openmp/LICENSE.TXT — „Apache License v2.0 with LLVM Exceptions (parafraza nagłówka)”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/vendor/cpp-httplib/CMakeLists.txt — „license_add_file("BoringSSL" "${boringssl_SOURCE_DIR}/LICENSE")”
  - https://github.com/ggml-org/llama.cpp/pull/18696 — „common : add --license to display embedded licenses”
- **Uwaga krytyka:** Ten sam brak: nie odnotowano osadzenia własnego LICENSE llama.cpp (CMakeLists.txt:198 przy b11559).

### `sidecar-llama-cuda`

- **Karta w programie:** MIT AND LicenseRef-NVIDIA-CUDA-EULA (MIT (llama.cpp); NVIDIA CUDA Toolkit EULA (cudart, cuBLAS)), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://github.com/ggml-org/llama.cpp/releases/tag/b11559 (dwa pliki: llama-b11559-bin-win-cuda-12.4-x64.zip, 265 207 128 B, SHA-256 4a885b80b26a213e86b774a580ac14fb210eba6220579051339c71b2bda1ecd7; cudart-llama-bin-win-cuda-12.4-x64.zip, 391 443 627 B, SHA-256 8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6; zgodne z data.rs)
- **Licencja u źródła:** llama.cpp: MIT, z tymi samymi komponentami CPU co pozostałe paczki (LLVM OpenMP, BoringSSL, cpp-httplib, nlohmann/json). Biblioteki cudart64__.dll, cublas64__.dll i cublasLt64_*.dll z CUDA Toolkit 12.4 objęte są umową NVIDIA CUDA Toolkit EULA (wersja 12.4, aktualizacja 12.01.2024; bieżąca wersja online to 13.4 z 26.01.2026).
- **Tekst licencji:** https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html (wersja dla CUDA 12.4); llama.cpp: https://github.com/ggml-org/llama.cpp/blob/b11559/LICENSE
- **Wydawca:** ggml-org (llama.cpp, publikuje oba archiwa na GitHubie); NVIDIA Corporation (biblioteki CUDA)
- **Ograniczenia i warunki:**
  - Skład archiwum cudart według release.yml b11559: workflow kopiuje z instalacji CUDA tylko cudart64__.dll, cublas64__.dll i cublasLt64_*.dll, bez pliku z umową NVIDIA. Archiwum llama-…-cuda-12.4 zawiera ggml-cuda.dll i dołożoną zawartość paczki CPU.
  - EULA §1.1.1: wolno rozpowszechniać tylko części SDK oznaczone jako redystrybuowalne, w postaci kodu wynikowego wbudowanego w aplikację spełniającą wymagania umowy.
  - EULA §1.1.2: aplikacja musi mieć istotną dodatkową funkcjonalność ponad części SDK, a z redystrybuowanych części może korzystać tylko ta aplikacja. Warunki, na jakich rozpowszechnia się aplikację, muszą być zgodne z umową i chronić prywatność, bezpieczeństwo i prawa użytkowników. O znanym lub podejrzewanym niezgodnym użyciu trzeba powiadomić NVIDIA na piśmie. Narzędzia nieoznaczone jako redystrybuowalne służą tylko do użytku wewnętrznego.
  - EULA Załącznik A uznaje za redystrybuowalne CUDA Runtime (Windows: cudart.dll, cudart_static.lib, cudadevrt.lib) i cuBLAS (Windows: cublas.dll, cublasLt.dll), także odmiany nazw z numerem wersji lub architektury (przykład w umowie: cudart64_90.dll). Pliki cudart64_12, cublas64_12 i cublasLt64_12 mieszczą się więc w tym opisie, wnioskując z brzmienia umowy.
  - EULA §1.2(1): nie wolno usuwać informacji o prawach autorskich. Według odczytanej strony umowa nie wymaga wprost dołączania jej kopii do aplikacji. Wymóg informacji „source code provided by NVIDIA” dotyczy tylko przykładowego kodu źródłowego, nie bibliotek.
  - Według data.rs program pobiera oba archiwa z wydania ggml-org na GitHubie, czyli biblioteki NVIDIA rozpowszechnia tam ggml-org. Czy Alfa, pobierając je na żądanie użytkownika albo dołączając do własnego pakietu, sama staje się rozpowszechniającym w rozumieniu EULA, tego nie ustalono (kwestia prawna).
  - Dla części llama.cpp obowiązują te same warunki MIT i komponentów wbudowanych co w paczce CPU (LLVM OpenMP Apache-2.0 WITH LLVM-exception, BoringSSL Apache-2.0).
- **Znak wodny:** Nie dotyczy, to serwer wnioskowania, nie silnik mowy.
- **Rozbieżności z kartą:**
  - Karta odsyła do bieżącej EULA (docs.nvidia.com/cuda/eula, obecnie v13.4 z 26.01.2026), a biblioteki pochodzą z CUDA 12.4, dla którego obowiązuje wersja z archiwum NVIDIA (v12.4, 12.01.2024). Istotne warunki redystrybucji w obu wersjach są zbliżone.
  - Karta mówi o „cudart, cuBLAS”, a archiwum zawiera też cuBLASLt (cublasLt64_12.dll). Ta biblioteka również jest redystrybuowalna według Załącznika A.
  - Karta nie wymienia komponentów wbudowanych w część llama.cpp (LLVM OpenMP, BoringSSL, cpp-httplib, nlohmann/json) i odsyła do LICENSE na master zamiast przy tagu b11559.
  - Archiwum cudart od ggml-org nie zawiera umowy NVIDIA (według skryptu budowania), więc kartę licencji Alfy warto oprzeć na wersji EULA 12.4.
- **Dowody:**
  - https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11559 — „cudart-llama-bin-win-cuda-12.4-x64.zip 391443627 8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6”
  - https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11559 — „llama-b11559-bin-win-cuda-12.4-x64.zip 265207128 4a885b80b26a213e86b774a580ac14fb210eba6220579051339c71b2bda1ecd7”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/.github/workflows/release.yml — „cudart64__.dll, cublas64__.dll, cublasLt64_*.dll kopiowane do .\build\bin\cudart\ (parafraza kroku robocopy)”
  - https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html — „v12.4; Last updated: January 12, 2024”
  - https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html — „Załącznik A: cudart.dll, cudart_static.lib, cudadevrt.lib; cublas.dll, cublasLt.dll”
  - https://docs.nvidia.com/cuda/eula/index.html — „v13.4; Last updated: January 26, 2026”
- **Uwaga krytyka:** Ten sam brak dotyczący osadzonego LICENSE llama.cpp. Część o NVIDIA się potwierdza: archiwum EULA 12.4 (12.01.2024), Załącznik A z cublas.dll i cublasLt.dll, przykład cudart64_90.dll oraz robocopy tylko cudart64__, cublas64__ i cublasLt64_* w release.yml:1280–1283.

### `sidecar-llama-cpu`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://github.com/ggml-org/llama.cpp/releases/tag/b11559 (plik llama-b11559-bin-win-cpu-x64.zip, 19 501 338 B, SHA-256 1228f133868c02a424bcf663a688e66d8c7d5499077649f305188f756f584377 według GitHuba, zgodne z data.rs)
- **Licencja u źródła:** MIT (LICENSE przy tagu b11559, prawa autorskie: The ggml authors, 2023–2026). Komponenty wbudowane według skryptów budowania: libomp.dll z LLVM OpenMP 20.1.8 i plikiem LICENSE-LLVM-OpenMP (Apache-2.0 WITH LLVM-exception, część kodu na dawnej licencji NCSA/MIT), BoringSSL 0.20260929.0 (Apache-2.0), cpp-httplib (MIT) i nlohmann/json (MIT).
- **Tekst licencji:** https://github.com/ggml-org/llama.cpp/blob/b11559/LICENSE
- **Wydawca:** ggml-org (prawa autorskie: The ggml authors)
- **Ograniczenia i warunki:**
  - MIT wymaga dołączenia informacji o prawach autorskich i treści zezwolenia do kopii programu.
  - Zadanie windows-cpu (release.yml b11559) pakuje cały katalog build\bin\Release (llama-server.exe, warianty ggml-cpu dla różnych procesorów i bibliotekę OpenMP) z opcjami GGML_OPENMP=ON, GGML_OPENMP_FETCH=ON i LLAMA_BUILD_BORINGSSL=ON. Skrypt CMake kopiuje libomp.dll i LICENSE-LLVM-OpenMP do katalogu wynikowego.
  - LLVM OpenMP: Apache-2.0 z wyjątkami LLVM. Plik licencji leży obok DLL, według skryptu budowania. Zawartości archiwum b11559 nie sprawdzano.
  - Licencje komponentów (m.in. BoringSSL, cpp-httplib) są osadzane w plikach wykonywalnych i wyświetlane opcją --license (PR #18696). Działania tej opcji w b11559 nie sprawdzano.
  - Skrypty publikacji nie kopiują głównego pliku LICENSE do archiwów Windows. Dokumentacja Alfy odnotowała pliki LICENSE-* w archiwach b6710, a dla b11559 tego nie sprawdzano.
- **Znak wodny:** Nie dotyczy, to serwer wnioskowania, nie silnik mowy.
- **Rozbieżności z kartą:**
  - Karta odsyła do LICENSE na gałęzi master zamiast przy tagu b11559, z którego pobiera program. Licencja przy tagu to też MIT.
  - Karta nie wymienia licencji komponentów dołączonych do paczki: libomp.dll i LLVM OpenMP (Apache-2.0 WITH LLVM-exception), BoringSSL (Apache-2.0), cpp-httplib (MIT) i nlohmann/json (MIT).
  - Karta podaje autora „Georgi Gerganov i współtwórcy”, a LICENSE mówi o prawach „The ggml authors” (różnica drobna).
- **Dowody:**
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/LICENSE — „MIT License; Copyright (c) 2023-2026 The ggml authors”
  - https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11559 — „llama-b11559-bin-win-cpu-x64.zip 19501338 1228f133868c02a424bcf663a688e66d8c7d5499077649f305188f756f584377”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/.github/workflows/release.yml — „7z a -snl llama-bin-win-cpu-${{ matrix.arch }}.zip .\build\bin\Release\*”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/ggml/src/CMakeLists.txt — „copy_if_different "${GGML_OPENMP_LICENSE}" "${GGML_OPENMP_RUNTIME_OUTPUT_DIR}/LICENSE-LLVM-OpenMP"”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/licenses/LICENSE-jsonhpp — „MIT License, Niels Lohmann (parafraza)”
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/b11559/vendor/cpp-httplib/LICENSE — „MIT License, yhirose 2017 (parafraza)”
- **Uwaga krytyka:** Niepełny opis informacji licencyjnych. Wynik mówi, że skrypty nie kopiują głównego pliku LICENSE do archiwum, i wymienia jako osadzone tylko licencje komponentów. Tymczasem główny CMakeLists.txt przy b11559 (wiersz 198) wywołuje license_add_file dla llama.cpp i jego LICENSE, więc tekst MIT samego llama.cpp też jest osadzony w binarkach (opcja --license). Ma to znaczenie dla wymogu MIT, by dołączyć informację o prawach autorskich. Pozostałe dane (sumy, rozmiary, OpenMP 20.1.8, BoringSSL 0.20260929.0, PR #18696 scalony 10.01.2026) się potwierdzają.

### `multilingual-e5-small`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/intfloat/multilingual-e5-small
- **Licencja u źródła:** MIT (tylko jako znacznik metadanych karty modelu: license: mit; w repozytorium HF nie ma pliku LICENSE)
- **Tekst licencji:** https://huggingface.co/intfloat/multilingual-e5-small/blob/main/README.md (nagłówek YAML); pełny tekst MIT z linią praw autorskich wydawcy: https://github.com/microsoft/unilm/blob/master/LICENSE
- **Wydawca:** intfloat (konto HF Liang Wanga, zespół E5 Microsoft Research; repozytorium microsoft/unilm/e5 odsyła do tego modelu). Model bazowy: microsoft/Multilingual-MiniLM-L12-H384 (MIT); tokenizer XLM-R (FacebookAI/xlm-roberta-base, MIT).
- **Ograniczenia i warunki:**
  - MIT: przy rozpowszechnianiu trzeba dołączyć informację o prawach autorskich i tekst licencji.
  - Karta modelu prosi o cytowanie raportu 'Multilingual E5 Text Embeddings' (2024). To prośba, nie warunek licencji.
  - Ograniczenia techniczne z karty: wejście do 512 tokenów, słabsze wyniki dla języków o małych zasobach.
  - Licencje danych treningowych nieustalone u źródła.
- **Znak wodny:** nie dotyczy (model wektorów tekstu, nie silnik mowy)
- **Rozbieżności z kartą:**
  - Brak rozbieżności co do samej licencji (MIT) ani zgody na użycie komercyjne.
  - Karta podaje ogólny adres opensource.org; u źródła licencja jest wyłącznie znacznikiem w nagłówku README na HF, bez pliku LICENSE i bez linii praw autorskich. Tekst MIT z linią 'Copyright (c) Microsoft Corporation' jest w microsoft/unilm/LICENSE (projekt nadrzędny E5). Warto podać w karcie oba adresy.
  - Autor na HF to konto 'intfloat', nie organizacja Microsoft. Przypisanie Microsoftowi opieram na tym, że repozytorium microsoft/unilm/e5 odsyła do modelu; strona arXiv 2402.05672 wymienia autorów bez afiliacji.
  - Karta nie wspomina modelu bazowego: microsoft/Multilingual-MiniLM-L12-H384 (MIT) z tokenizerem XLM-R (MIT). Obie licencje są zgodne z MIT.
  - Dane treningowe według karty modelu: mC4, CC News, NLLB, Wikipedia, Reddit, S2ORC, Stack Exchange, xP3, MS MARCO, NQ, TriviaQA, SQuAD, Quora, MIRACL i inne. Karta modelu nie omawia licencji tych zbiorów i nie sprawdzałem ich warunków (np. MS MARCO).
  - Spoza karty licencji: program pobiera pliki z resolve/main bez przypiętej rewizji i bez sum (sha256: None w crates/lib-embed/src/catalog.rs). Stan u źródła 2026-10-21: commit 614241f622f53c4eeff9890bdc4f31cfecc418b3; onnx/model.onnx ma 470268510 B, sha256 ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665; tokenizer.json ma 17082730 B, sha256 0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39.
- **Dowody:**
  - https://huggingface.co/intfloat/multilingual-e5-small/raw/main/README.md — „license: mit”
  - https://huggingface.co/api/models/intfloat/multilingual-e5-small/tree/main — „brak pliku LICENSE; pliki: tokenizer.json 17082730 B, katalog onnx/ (model.onnx 470268510 B)”
  - https://github.com/microsoft/unilm/blob/master/LICENSE — „Copyright (c) Microsoft Corporation”
  - https://huggingface.co/microsoft/Multilingual-MiniLM-L12-H384/raw/main/README.md — „license: mit”
  - https://huggingface.co/api/models/FacebookAI/xlm-roberta-base — „license:mit”

### `paraphrase-multilingual-minilm-l12-v2`

- **Karta w programie:** Apache-2.0 (Apache License 2.0), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2
- **Licencja u źródła:** Apache-2.0 (znacznik metadanych karty: license: apache-2.0; w repozytorium HF nie ma pliku LICENSE ani NOTICE)
- **Tekst licencji:** https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2/blob/main/README.md (nagłówek YAML); tekst licencji: https://www.apache.org/licenses/LICENSE-2.0
- **Wydawca:** Organizacja sentence-transformers na HF. Model wytrenował zespół UKP Lab TU Darmstadt (Nils Reimers, Iryna Gurevych, praca Sentence-BERT). Od 22.10.2025 bibliotekę i projekt utrzymuje Hugging Face.
- **Ograniczenia i warunki:**
  - Apache-2.0: przy rozpowszechnianiu trzeba dołączyć kopię licencji i oznaczyć zmienione pliki (NOTICE u źródła nie istnieje).
  - Karta modelu prosi o cytowanie Sentence-BERT (arXiv 1908.10084). To prośba, nie warunek licencji.
  - Ograniczenie techniczne: max_seq_length 128.
  - Dane treningowe i ich licencje nieustalone u źródła.
- **Znak wodny:** nie dotyczy (model wektorów tekstu, nie silnik mowy)
- **Rozbieżności z kartą:**
  - Licencja (Apache-2.0) i zgoda na użycie komercyjne zgodne z kartą.
  - U źródła licencja to tylko znacznik w README, bez pliku LICENSE i bez NOTICE, więc nie ma pliku NOTICE do przenoszenia.
  - Autor w karcie jest aktualny tylko historycznie. 22.10.2025 UKP Lab przekazał utrzymanie i dalszy rozwój Sentence Transformers firmie Hugging Face. Proponuję w karcie: 'sentence-transformers (pierwotnie UKP Lab, TU Darmstadt; od 10.2025 utrzymywany przez Hugging Face)'.
  - Karta modelu nie podaje modelu bazowego (metadane bez base_model; config: BertModel, vocab 250037). Dokumentacja sbert.net opisuje go jako wielojęzyczną wersję paraphrase-MiniLM-L12-v2, trenowaną na danych równoległych w 50+ językach. Danych treningowych i ich licencji karta nie opisuje, więc są nieustalone.
  - Spoza karty licencji: crates/lib-embed/src/catalog.rs podaje rozmiar tokenizer.json 17 MB, a u źródła plik ma 9081518 B (~8,7 MiB; sha256 2c3387be76557bd40970cec13153b3bbf80407865484b209e655e5e4729076b8). Wartość wygląda na skopiowaną z E5.
  - Spoza karty licencji: pobieranie z resolve/main bez przypiętej rewizji i sum. Stan u źródła: commit e8f8c211226b894fcb81acc59f3b34ba3efd5f42; onnx/model.onnx ma 470301610 B, sha256 10f7a088420252b26caf819236ca2c9d2987afd0fc06fec7553b542a5655a05a.
- **Dowody:**
  - https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2/raw/main/README.md — „license: apache-2.0”
  - https://huggingface.co/api/models/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2/tree/main — „brak plików LICENSE/NOTICE; tokenizer.json size 9081518”
  - https://www.informatik.tu-darmstadt.de/ukp/ukp_home/ukp_news_details_327040.en.jsp — „2025/10/22: przekazanie utrzymania Sentence Transformers firmie Hugging Face (streszczenie)”
  - https://www.sbert.net/docs/sentence_transformer/pretrained_models.html — „wielojęzyczna wersja paraphrase-MiniLM-L12-v2, dane równoległe 50+ języków (streszczenie)”

### `whisper-large-v3-turbo-q5_0`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/ggerganov/whisper.cpp (plik ggml-large-v3-turbo-q5_0.bin); wagi pierwotne: https://github.com/openai/whisper (model 'turbo') oraz https://huggingface.co/openai/whisper-large-v3-turbo
- **Licencja u źródła:** Wagi OpenAI: MIT (README i LICENSE w github.com/openai/whisper; karta HF openai/whisper-large-v3-turbo: license: mit). Konwersja ggml: MIT (znacznik metadanych repozytorium HF ggerganov/whisper.cpp, bez pliku LICENSE). Kod konwertera whisper.cpp: MIT.
- **Tekst licencji:** https://github.com/openai/whisper/blob/main/LICENSE ; https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE ; https://huggingface.co/ggerganov/whisper.cpp/blob/main/README.md (nagłówek YAML)
- **Wydawca:** OpenAI (wagi; Copyright (c) 2022 OpenAI). Plik .bin wystawia konto HF 'ggerganov' (Georgi Gerganov). Konwerter convert-pt-to-ggml.py pochodzi z projektu ggml-org/whisper.cpp (Copyright (c) 2023-2026 The ggml authors).
- **Ograniczenia i warunki:**
  - MIT: przy rozpowszechnianiu trzeba dołączyć 'Copyright (c) 2022 OpenAI' i tekst licencji. Dla konwersji ggml warto dołączyć też informację z LICENSE whisper.cpp.
  - Karta modelu OpenAI (zalecenia, nie warunki licencji) odradza transkrypcję nagrań osób bez ich zgody, użycie w dziedzinach wysokiego ryzyka (np. podejmowanie decyzji) i wnioskowanie o cechach ludzi. Ostrzega też przed użyciem do inwigilacji.
  - Licencje danych treningowych OpenAI nieustalone u źródła (karta ich nie podaje).
- **Znak wodny:** nie dotyczy (rozpoznawanie mowy, nie generowanie)
- **Rozbieżności z kartą:**
  - Licencja MIT dla wag i konwersji zgodna z kartą. Zgoda na użycie komercyjne też zgodna.
  - Plik .bin leży w prywatnej przestrzeni HF 'ggerganov/whisper.cpp' (autor: ggerganov, ostatnia zmiana 2024-10-29), a nie w organizacji ggml-org. Karta przypisuje zapis GGML 'ggml-org'. Do konwertera to pasuje, do miejsca publikacji nie.
  - Karta linkuje tylko LICENSE OpenAI. MIT konwersji ggml jest u źródła wyłącznie znacznikiem metadanych repozytorium HF, bez pliku LICENSE. Najbliższy tekst z linią praw autorskich to LICENSE whisper.cpp. Warto dodać ten adres do karty.
  - Model 'turbo' to według README OpenAI zoptymalizowana wersja large-v3, nietrenowana do tłumaczenia. Karta HF openai/whisper-large-v3-turbo wskazuje base_model openai/whisper-large-v3. Licencja wszędzie MIT.
  - Spoza karty licencji: suma pliku nieprzypięta w data_speech.rs (None). Stan u źródła: ggml-large-v3-turbo-q5_0.bin ma 574041195 B (~547,4 MiB, zgodnie z 547 w katalogu), LFS sha256 394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2, commit repozytorium 5359861c739e955e79d9a303bcbc70fb988958b1. Pobieranie z resolve/main bez przypiętej rewizji.
- **Dowody:**
  - https://github.com/openai/whisper/blob/main/README.md — „Whisper's code and model weights are released under the MIT License.”
  - https://github.com/openai/whisper/blob/main/LICENSE — „Copyright (c) 2022 OpenAI”
  - https://huggingface.co/openai/whisper-large-v3-turbo/raw/main/README.md — „license: mit”
  - https://huggingface.co/ggerganov/whisper.cpp/raw/main/README.md — „license: mit”
  - https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE — „Copyright (c) 2023-2026 The ggml authors”
  - https://github.com/ggml-org/whisper.cpp/blob/master/models/README.md — „modele PyTorch OpenAI konwertowane do ggml skryptem convert-pt-to-ggml.py (streszczenie)”
- **Uwaga krytyka:** Określenie, że plik leży w prywatnej przestrzeni HF, wprowadza w błąd. ggerganov/whisper.cpp to publiczne repozytorium na koncie użytkownika (API: author ggerganov, lastModified 29.10.2024, license mit). Prywatne nie jest. Przy okazji: wynik podaje informację o prawach autorskich z LICENSE whisper.cpp na master (2023–2026), a sidecar-whisper-cpu podaje ją przy tagu v1.8.1 (2023–2024). Lepiej przyjąć jedną wersję.

### `whisper-small-q5_1`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/ggerganov/whisper.cpp (plik ggml-small-q5_1.bin); wagi pierwotne: https://github.com/openai/whisper (model 'small')
- **Licencja u źródła:** Wagi OpenAI z repozytorium GitHub: MIT. Konwersja ggml: MIT (znacznik metadanych HF ggerganov/whisper.cpp). Uwaga: osobna karta HF openai/whisper-small (wersja Transformers) ma znacznik license: apache-2.0.
- **Tekst licencji:** https://github.com/openai/whisper/blob/main/LICENSE ; https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE ; https://huggingface.co/ggerganov/whisper.cpp/blob/main/README.md (nagłówek YAML)
- **Wydawca:** OpenAI (wagi; Copyright (c) 2022 OpenAI). Plik .bin wystawia konto HF 'ggerganov' (Georgi Gerganov). Konwerter pochodzi z ggml-org/whisper.cpp (The ggml authors).
- **Ograniczenia i warunki:**
  - MIT: przy rozpowszechnianiu trzeba dołączyć 'Copyright (c) 2022 OpenAI' i tekst licencji, oraz informację z LICENSE whisper.cpp dla konwersji.
  - Karta modelu OpenAI (zalecenia, nie warunki licencji) odradza transkrypcję nagrań osób bez zgody, użycie w decyzjach wysokiego ryzyka i klasyfikowanie ludzi. Ostrzega też przed inwigilacją.
  - Licencje danych treningowych nieustalone u źródła.
- **Znak wodny:** nie dotyczy (rozpoznawanie mowy, nie generowanie)
- **Rozbieżności z kartą:**
  - MIT zgodne z kartą dla ścieżki, z której program bierze plik: oryginalny checkpoint PyTorch OpenAI (MIT według README i LICENSE GitHub) przekonwertowany do ggml i oznaczony w repozytorium HF jako MIT.
  - Niejednoznaczność u źródła, nie błąd karty: karta HF openai/whisper-small (checkpoint Transformers) ma znacznik apache-2.0, a repozytorium GitHub OpenAI deklaruje MIT dla wag. Według models/README whisper.cpp pliki ggml powstają z oryginalnych modeli PyTorch OpenAI, więc właściwą podstawą jest MIT. Obie licencje pozwalają na użycie komercyjne. Warto dopisać tę uwagę w karcie.
  - Jak przy turbo: plik leży w przestrzeni 'ggerganov', nie 'ggml-org'. Karta linkuje tylko LICENSE OpenAI, bez LICENSE whisper.cpp.
  - Spoza karty licencji: suma nieprzypięta. Stan u źródła: ggml-small-q5_1.bin ma 190085487 B (~181,3 MiB, zgodnie z 181 w katalogu), LFS sha256 ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb, commit 5359861c739e955e79d9a303bcbc70fb988958b1.
- **Dowody:**
  - https://github.com/openai/whisper/blob/main/LICENSE — „Copyright (c) 2022 OpenAI”
  - https://huggingface.co/ggerganov/whisper.cpp/raw/main/README.md — „license: mit”
  - https://huggingface.co/openai/whisper-small/raw/main/README.md — „license: apache-2.0”
  - https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/main — „ggml-small-q5_1.bin size 190085487, brak pliku LICENSE”
  - https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE — „Copyright (c) 2023-2026 The ggml authors”
- **Uwaga krytyka:** Ta sama nieścisłość (przestrzeń nazwana prywatną). Reszta się potwierdza: openai/whisper-small na HF ma apache-2.0, README OpenAI na GitHubie mówi o MIT dla kodu i wag, a rozmiar i LFS sha256 pliku się zgadzają.

### `qwen3-asr-1.7b-q8`

- **Karta w programie:** Apache-2.0 (Apache License 2.0), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF (przypięta rewizja 36a678687ba7d07a74ca70ccb0e36902e005fb80 = obecny main, lastModified 2026-04-13; pliki Qwen3-ASR-1.7B-Q8_0.gguf i mmproj-Qwen3-ASR-1.7B-Q8_0.gguf są na liście plików). Model bazowy: https://huggingface.co/Qwen/Qwen3-ASR-1.7B
- **Licencja u źródła:** Wydawca GGUF (ggml-org) nie podaje licencji: karta nie ma pola license, w tagach nie ma license:..., w repozytorium nie ma pliku LICENSE. Model bazowy Qwen/Qwen3-ASR-1.7B ma Apache-2.0 (metadane karty HF, bez pliku LICENSE w repozytorium wag). Repozytorium kodu QwenLM/Qwen3-ASR ma Apache-2.0, „Copyright 2026 Alibaba Cloud”.
- **Tekst licencji:** https://huggingface.co/Qwen/Qwen3-ASR-1.7B/raw/main/README.md (front matter license: apache-2.0); https://github.com/QwenLM/Qwen3-ASR/blob/main/LICENSE
- **Wydawca:** Model: Qwen (Alibaba Cloud). Zapis GGUF: ggml-org (konwersja przez convert_hf_to_gguf.py z llama.cpp).
- **Ograniczenia i warunki:**
  - Apache-2.0 §4: przy rozpowszechnianiu trzeba dołączyć kopię licencji i zachować informacje o prawach autorskich (Copyright 2026 Alibaba Cloud). Pliku NOTICE nie widać ani w repozytorium wag, ani w repozytorium GGUF.
  - Apache-2.0 §6: licencja nie daje praw do znaku „Qwen” (wolno go użyć tylko do opisu pochodzenia).
  - Apache-2.0 §4(b): zmienione pliki mają nieść informację o zmianie. ggml-org opisuje konwersję jednym zdaniem w README, wymóg dotyczy wydawcy GGUF.
  - Danych treningowych nie ujawniono: karta mówi tylko o „large-scale speech training data” i o budowie na zdolnościach Qwen3-Omni. Licencji Qwen3-Omni nie sprawdzałam.
  - Karta wag u Qwen nie ma dodatkowych zasad użycia (AUP) poza tagiem Apache-2.0. Prosi tylko o cytowanie raportu arXiv:2601.21337, co jest prośbą, a nie warunkiem licencji.
- **Znak wodny:** Nie dotyczy (model rozpoznawania mowy, nie TTS).
- **Rozbieżności z kartą:**
  - Karta przypisuje Apache-2.0 także zapisowi GGUF z ggml-org, ale ggml-org licencji nie deklaruje. Apache-2.0 wynika tylko z modelu bazowego, czyli z dziedziczenia, a nie z deklaracji wydawcy pliku, który Alfa faktycznie pobiera.
  - url w karcie prowadzi do ogólnego tekstu Apache na apache.org, a nie do miejsca deklaracji u źródła (karta HF Qwen/Qwen3-ASR-1.7B lub LICENSE w QwenLM/Qwen3-ASR).
  - Notatka karty nie podaje właściciela praw z pliku LICENSE („Copyright 2026 Alibaba Cloud”), a tę informację trzeba zachować przy wskazaniu autora.
  - Repozytorium wag Qwen na HF nie zawiera pliku LICENSE (jest tylko tag licencji), więc tekst licencji do instalacji obok modelu trzeba wziąć z GitHuba albo z apache.org.
  - Plików docs/plan/19-specyfikacja-glosu-delty.md, na które powołuje się karta, nie weryfikowałam.
- **Dowody:**
  - https://huggingface.co/api/models/ggml-org/Qwen3-ASR-1.7B-GGUF — „cardData: {"base_model":"Qwen/Qwen3-ASR-1.7B"} (w tagach brak license:...)”
  - https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF — „"converted from Qwen/Qwen3-ASR-1.7B to GGUF using convert_hf_to_gguf.py"”
  - https://huggingface.co/Qwen/Qwen3-ASR-1.7B/raw/main/README.md — „"license: apache-2.0"”
  - https://raw.githubusercontent.com/QwenLM/Qwen3-ASR/main/LICENSE — „"Copyright 2026 Alibaba Cloud"”
  - https://huggingface.co/Qwen/Qwen3-ASR-1.7B — „"Both leverage large-scale speech training data"”

### `smart-turn-v3.2`

- **Karta w programie:** BSD-2-Clause (BSD 2-Clause License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://huggingface.co/pipecat-ai/smart-turn-v3 (przypięta rewizja f766f81d3cfdf7737ac64aad813d91bbfd56bf93 = obecny main; plik smart-turn-v3.2-cpu.onnx jest na liście plików). Kod i licencja: https://github.com/pipecat-ai/smart-turn
- **Licencja u źródła:** BSD-2-Clause: tag license:bsd-2-clause na HF oraz plik LICENSE w repozytorium GitHub autora („BSD 2-Clause License”, „Copyright (c) 2024–2025, Daily”). W repozytorium HF nie ma pliku LICENSE.
- **Tekst licencji:** https://github.com/pipecat-ai/smart-turn/blob/main/LICENSE
- **Wydawca:** Daily (organizacja pipecat-ai / Pipecat na HF i GitHubie)
- **Ograniczenia i warunki:**
  - BSD-2-Clause: przy dystrybucji binarnej trzeba odtworzyć informację o prawach autorskich, warunki i wyłączenie odpowiedzialności w dokumentacji lub materiałach towarzyszących.
  - Dane treningowe pipecat-ai/smart-turn-data-v3.1-train/test (karta HF) i v3.2-train (README na GitHubie) mają na HF licencję CC-BY-4.0. Dotyczy ona zbiorów, nie wag, ale autor wymienia te dane jako podstawę modelu.
  - Część danych dostarczyły Liva AI, Midcentury i MundoAI. Ich warunków autor nie podaje (nie ustalono).
  - BSD-2-Clause nie zawiera wyraźnej licencji patentowej.
- **Znak wodny:** Nie dotyczy (model wykrywania końca wypowiedzi, nie TTS).
- **Rozbieżności z kartą:**
  - url w karcie prowadzi do ogólnego tekstu na opensource.org, a nie do pliku LICENSE autora (github.com/pipecat-ai/smart-turn/blob/main/LICENSE).
  - Karta nie podaje brzmienia informacji o prawach autorskich („Copyright (c) 2024–2025, Daily”), a BSD-2 wymaga jej odtworzenia. Spostrzeżenie: w voice_runtime.rs pozycja pobiera tylko plik .onnx (Install::Files), bez tekstu licencji, inaczej niż ONNX Runtime i DirectML.
  - Kraj (USA) z karty nie wynika ze stron źródłowych, które czytałam (nie ustalono u źródła).
  - Karta nie wspomina o danych treningowych na CC-BY-4.0.
  - Niespójność u źródła: karta HF wymienia zbiory v3.1, a README na GitHubie zbiory v3.2.
- **Dowody:**
  - https://huggingface.co/pipecat-ai/smart-turn-v3/raw/main/README.md — „"license: bsd-2-clause"”
  - https://raw.githubusercontent.com/pipecat-ai/smart-turn/main/LICENSE — „"Copyright (c) 2024–2025, Daily"”
  - https://github.com/pipecat-ai/smart-turn — „"This is a truly open model (BSD 2-clause license)."”
  - https://huggingface.co/api/datasets/pipecat-ai/smart-turn-data-v3.1-train — „"license:cc-by-4.0"”
  - https://huggingface.co/api/datasets/pipecat-ai/smart-turn-data-v3.2-train — „"license:cc-by-4.0"”

### `onnxruntime-cpu`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://github.com/microsoft/onnxruntime/releases/tag/v1.23.1 (zasób onnxruntime-win-x64-1.23.1.zip, 78 099 392 B, zgodny z rozmiarem przypiętym w voice_runtime.rs; opublikowano 2025-10-08)
- **Licencja u źródła:** MIT („Copyright (c) Microsoft Corporation”)
- **Tekst licencji:** https://github.com/microsoft/onnxruntime/blob/v1.23.1/LICENSE
- **Wydawca:** Microsoft
- **Ograniczenia i warunki:**
  - MIT: informację o prawach autorskich i tekst licencji trzeba dołączyć do kopii.
  - ThirdPartyNotices.txt wymienia składniki na innych licencjach, m.in. Eigen (MPL-2.0), Intel MKL (Intel Simplified Software License, nie OSI), protobuf (BSD-3), ONNX (Apache-2.0). Przeczytałam tylko ok. 30% pliku, a które składniki są faktycznie w onnxruntime.dll — nie ustalono.
  - Telemetria: według Privacy.md dla v1.23.1 oficjalne buildy mają telemetrię domyślnie WŁĄCZONĄ, a dostawca Windows używa TraceLogging. Wyłącza ją DisableTelemetryEvents() (OrtEnv/OrtApi).
  - Spostrzeżenie z repozytorium (tylko odczyt): grep w *.rs nie znalazł wywołań DisableTelemetryEvents ani with_telemetry. Projekt deklaruje „Telemetria: Brak” (docs/PLAN.md:71). Domyślnego ustawienia crate'a ort nie sprawdzałam — do potwierdzenia przez człowieka.
- **Znak wodny:** Nie dotyczy (biblioteka obliczeń, nie TTS).
- **Rozbieżności z kartą:**
  - url w karcie wskazuje gałąź main, a nie tag v1.23.1. Treść jest ta sama (MIT, ten sam właściciel praw), ale do zapisu lepiej przypiąć tag wersji.
  - Karta nie wspomina, że oficjalne buildy Microsoftu mają domyślnie włączoną telemetrię (TraceLogging na Windows), co dotyczy zasady „bez telemetrii”.
- **Dowody:**
  - https://github.com/microsoft/onnxruntime/blob/v1.23.1/LICENSE — „"Copyright (c) Microsoft Corporation" (MIT License)”
  - https://github.com/microsoft/onnxruntime/blob/v1.23.1/docs/Privacy.md — „"Telemetry is turned ON by default in the official builds"”
  - https://github.com/microsoft/onnxruntime/blob/v1.23.1/docs/Privacy.md — „"The Windows provider uses the TraceLogging API for its implementation."”
  - https://onnxruntime.ai/docs/api/csharp/api/Microsoft.ML.OnnxRuntime.OrtEnv.html — „"currently only official Windows ORT builds have telemetry collection capabilities"”
  - https://api.github.com/repos/microsoft/onnxruntime/releases/tags/v1.23.1 — „onnxruntime-win-x64-1.23.1.zip: 78099392”
- **Uwaga krytyka:** Telemetria została oznaczona jako do potwierdzenia, choć da się ją rozstrzygnąć z kodu. W ort 2.0.0-rc.10 (wersja przypięta w crates/lib-smart-turn/Cargo.toml i crates/lib-supertonic/Cargo.toml) EnvironmentBuilder::new() ustawia telemetry: true, a commit() przy tej wartości wywołuje EnableTelemetryEvents (źródło: docs.rs/crate/ort/2.0.0-rc.10/source/src/environment.rs). Alfa woła ort::init_from(dll).commit() bez with_telemetry(false) (crates/lib-smart-turn/src/main.rs:44, crates/lib-supertonic/src/main.rs:62), więc z oficjalną biblioteką Microsoftu zdarzenia telemetrii są jawnie włączone. Piper robi odwrotnie: wywołuje DisableTelemetryEvents (src/cpp/piper.cpp:266 przy tagu 2023.11.14-2). Do tego cytat z Privacy.md nie jest dosłowny. Oryginał mówi o oficjalnych buildach rozpowszechnianych w repozytoriach pakietów i o tym, że dane trafiają do Microsoftu przez TraceLogging systemu zależnie od zgody użytkownika. Wynik pomija oba zastrzeżenia.

### `onnxruntime-directml`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML/1.23.0 (wydana 2025-09-29; zależność native: Microsoft.AI.DirectML >= 1.15.4; najnowsza wersja na NuGet to 1.24.4 z 17.03.2026)
- **Licencja u źródła:** MIT. Strona pakietu nie pokazuje wyrażenia SPDX, tylko link „License Info”; plik licencji to MIT, „Copyright (c) Microsoft Corporation”.
- **Tekst licencji:** https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML/1.23.0/License
- **Wydawca:** Microsoft (właściciele na NuGet: Microsoft, Microsoft AI Infra and Tools team)
- **Ograniczenia i warunki:**
  - MIT: kopia licencji i informacja o prawach autorskich przy kopiach.
  - Wymaga Microsoft.AI.DirectML (>= 1.15.4), który ma własną, zamkniętą licencję Microsoftu (osobna pozycja).
  - Telemetria jak w oficjalnych buildach ONNX Runtime z repozytoriów pakietów (domyślnie włączona, TraceLogging na Windows, wyłączana przez DisableTelemetryEvents).
  - Dostawca DirectML w ONNX Runtime jest w trybie „sustained engineering”, a nowe funkcje trafiają do WinML (to stan wsparcia, nie warunek licencji).
- **Znak wodny:** Nie dotyczy (biblioteka obliczeń, nie TTS).
- **Rozbieżności z kartą:**
  - Co do licencji brak rozbieżności. url karty wskazuje LICENSE na GitHubie (main), a nie stronę licencji pakietu NuGet 1.23.0, z którego Alfa pobiera plik. Treść jest ta sama (MIT).
  - Karta nie wspomina o domyślnie włączonej telemetrii oficjalnych buildów ani o trybie „sustained engineering” DirectML.
  - Pliku docs/ADR/0028, na który powołuje się karta, nie weryfikowałam.
- **Dowody:**
  - https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML/1.23.0/License — „"Copyright (c) Microsoft Corporation" (MIT License)”
  - https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML/1.23.0 — „native: Microsoft.AI.DirectML (>= 1.15.4)”
  - https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html — „"Note: DirectML is in sustained engineering."”
- **Uwaga krytyka:** Ten sam brak co w onnxruntime-cpu. Biblioteka z DirectML działa w tym samym procesie alfa-supertonic, który przez domyślne ustawienie ort wywołuje EnableTelemetryEvents. Wynik opisuje tylko domyślne zachowanie oficjalnych buildów i nie mówi, że Alfa sama włącza telemetrię.

### `directml`

- **Karta w programie:** LicenseRef-Microsoft-DirectML (Microsoft Software License Terms — Microsoft DirectML), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4 (właściciele: Microsoft, DirectML; ostatnia aktualizacja 10/28/2024; 1.15.4 to najnowsza wersja; strona projektu https://aka.ms/DirectML)
- **Licencja u źródła:** Własna licencja Microsoftu, nie MIT: „MICROSOFT SOFTWARE LICENSE TERMS — MICROSOFT DIRECTX MACHINE LEARNING (DIRECTML)”. Repozytorium GitHub microsoft/DirectML ma licencję MIT, ale README nie odnosi jej do binariów z NuGet.
- **Tekst licencji:** https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License
- **Wydawca:** Microsoft
- **Ograniczenia i warunki:**
  - §1(a): instalacja i użycie wyłącznie na Windows i Xbox. Kopiowanie i rozpowszechnianie dozwolone w aplikacjach i usługach, które się tworzy (narzędzia ML, gry).
  - §3(e): nie wolno rozpowszechniać poza §1 ani udostępniać biblioteki jako osobnej oferty (stand-alone).
  - §3(a)-(d): zakaz obchodzenia ograniczeń technicznych, inżynierii wstecznej i dekompilacji, usuwania lub zmieniania informacji Microsoftu i dostawców, użycia niezgodnego z prawem lub do tworzenia złośliwego oprogramowania.
  - §2(a): oprogramowanie może zbierać dane o użytkowniku i użyciu oraz wysyłać je do Microsoftu. Część scenariuszy da się wyłączyć („many… but not all”) według dokumentacji produktu, a samo użycie oznacza zgodę. Dokumentacji wyłączenia dla pakietu redystrybucyjnego nie znalazłam (nie ustalono).
  - §2(a): jeśli aplikacja włącza funkcje zbierania danych od swoich użytkowników, trzeba przestrzegać prawa, w tym dać im odpowiednie informacje.
  - §6: oprogramowanie może samo sprawdzać, pobierać i instalować aktualizacje. Czy dotyczy to samej DLL z NuGet — nie ustalono.
  - §4 kontrola eksportu; §7 arbitraż (USA) i zrzeczenie się pozwów zbiorowych; §12 odpowiedzialność ograniczona do 5 USD; §10 odrębne postanowienia dla Australii, Kanady, Niemiec i Austrii.
  - DirectML jest w trybie utrzymania: tylko poprawki bezpieczeństwa i zgodności, bez nowych funkcji. Dla Windows 11 24H2+ Microsoft poleca Windows ML.
- **Znak wodny:** Nie dotyczy (biblioteka obliczeń GPU, nie TTS).
- **Rozbieżności z kartą:**
  - Licencja i odwołania (§1a redystrybucja, §2a dane diagnostyczne) zgadzają się z kartą.
  - attribution=true w karcie jest surowsze niż tekst: licencja nie wymaga wskazania autora ani dołączenia tekstu licencji, zakazuje tylko usuwania informacji Microsoftu (§3c). Instalowanie LICENSE-DirectML.txt i ThirdPartyNotices nie szkodzi.
  - Karta nie wspomina o ograniczeniu do Windows/Xbox (§1a), zakazie dystrybucji jako osobnej oferty (§3e), klauzuli automatycznych aktualizacji (§6) ani o trybie utrzymania DirectML.
  - Pytanie karty, czy zbieranie danych da się wyłączyć, pozostaje otwarte: licencja odsyła do „product documentation”, której dla pakietu redystrybucyjnego nie znalazłam.
  - Spostrzeżenie (bez oceny prawnej): Alfa pobiera pakiet z api.nuget.org na komputerze użytkownika, a nie dołącza go do instalatora, więc kwestia redystrybucji z §1a może jej nie dotyczyć — do oceny przez człowieka.
- **Dowody:**
  - https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License — „"MICROSOFT DIRECTX MACHINE LEARNING (DIRECTML)"”
  - https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License — „"and solely for use on Windows and Xbox." (§1a)”
  - https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License — „"provide the software as a stand-alone offering for others to use," (§3e)”
  - https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License — „"The software may collect information about you and your use of the software" (§2a)”
  - https://www.nuget.org/packages/Microsoft.AI.DirectML/1.15.4/License — „"You may opt-out of many of these scenarios, but not all" (§2a)”
  - https://github.com/microsoft/DirectML — „"DirectML is in maintenance mode."”

### `supertonic-3`

- **Karta w programie:** OpenRAIL-M AND MIT (OpenRAIL-M (wagi modelu i barwy głosów); MIT (kod przykładowy)), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://huggingface.co/supertone-oss-archive/supertonic-3 (rewizja przypięta w katalogu: aafc6e32416a594460b32413efc49d7fe4ce6d46)
- **Licencja u źródła:** Wagi ONNX, konfiguracja i barwy F1–F5/M1–M5: BigScience Open RAIL-M z 18.08.2022 (w metadanych HF: openrail; lista SPDX 3.29.0 nie ma dla niej identyfikatora). Kod przykładowy: MIT (Copyright (c) 2025 Supertone Inc.).
- **Tekst licencji:** https://huggingface.co/supertone-oss-archive/supertonic-3/blob/aafc6e32416a594460b32413efc49d7fe4ce6d46/LICENSE ; kod: https://github.com/supertone-oss-archive/supertonic/blob/main/LICENSE
- **Wydawca:** Supertone Inc. (Korea Południowa). Przestrzeń HF supertone-oss-archive nie jest zweryfikowana na HF, ale README repozytorium Supertone na GitHubie wskazuje ją jako oficjalne archiwum wag, a github.com/supertone-inc/supertonic przekierowuje na supertone-oss-archive/supertonic. Oryginał Supertone/supertonic-3 na HF nadal istnieje; plik LICENSE ma w obu miejscach tę samą sumę SHA-256 (0d944a91…).
- **Ograniczenia i warunki:**
  - Załącznik A Open RAIL-M zakazuje m.in.: podszywania się pod inne osoby (deepfake) bez ich zgody (pkt g), rozpowszechniania treści bez jasnej informacji, że wygenerowała je maszyna (pkt e), porad medycznych i interpretacji wyników medycznych (pkt l), dezinformacji szkodzącej innym (pkt c), w pełni automatycznych decyzji naruszających prawa osoby (pkt h), a także zastosowań w wymiarze sprawiedliwości, organach ścigania, imigracji i azylu (pkt m).
  - Pkt 4a: przy rozpowszechnianiu ograniczenia z pkt 5 trzeba wpisać jako wiążące postanowienie do umowy lub licencji, na której Alfa jest rozpowszechniana, i powiadomić odbiorców; pkt 5: trzeba wymagać od użytkowników przestrzegania tych ograniczeń.
  - Pkt 4b–4d: odbiorca dostaje kopię licencji, zmienione pliki dostają widoczną informację o zmianie, noty o prawach autorskich i autorstwie zostają zachowane.
  - Pkt 7: licencjodawca zastrzega sobie prawo ograniczania użycia (także zdalnie) i aktualizowania modelu, a licencjobiorca ma dokładać rozsądnych starań, by używać najnowszej wersji. Repozytorium jest zarchiwizowane, więc w praktyce najnowsza jest wersja przypięta.
  - Dane treningowe nie są objęte licencją (definicja „Data”) i źródło ich nie opisuje; nie podaje też, skąd pochodzą barwy F1–M5.
  - Repozytorium jest zarchiwizowane: bez poprawek, łatek bezpieczeństwa i wsparcia; Voice Builder wyłączono po 31.08.2026.
- **Znak wodny:** Brak. Producent nie dodaje znaku wodnego do mowy i go nie wymaga. Nie ma o nim wzmianki w README (HF i GitHub), w LICENSE ani w 52 plikach tekstowych i źródłowych repozytorium supertone-oss-archive/supertonic (przeszukane 2026-10-21). Licencja wymaga za to jawnego oznaczenia, że treść wygenerowała maszyna, gdy jest rozpowszechniana (Załącznik A, pkt e). To obowiązek ujawnienia, a nie znak wodny.
- **Rozbieżności z kartą:**
  - Pole id karty „OpenRAIL-M AND MIT” nie jest poprawnym wyrażeniem SPDX: lista SPDX 3.29.0 nie ma identyfikatora dla Open RAIL-M. Poprawniej np. „LicenseRef-BigScience-OpenRAIL-M AND MIT”. Pełna nazwa u źródła: BigScience Open RAIL-M z 18.08.2022.
  - Notatka karty mówi tylko o „przekazaniu dalej kopii licencji i listy zakazów”. Licencja (pkt 4a i 5) wymaga więcej: wpisania zakazów jako wiążącego postanowienia do licencji lub EULA Alfy i wymagania ich przestrzegania od użytkowników. Tego w karcie brakuje.
  - Karta nie wspomina pkt 7: zastrzeżenia zdalnych ograniczeń i aktualizacji oraz obowiązku starań o używanie najnowszej wersji.
  - Datę archiwizacji 2026-09-09 trzeba traktować jako przybliżoną, bo źródło nie podaje jej wprost. Ostatni push na GitHubie: 2026-09-09T06:54Z; repozytorium HF archiwum utworzono 2026-09-08; zapowiedź archiwizacji pochodzi z 23.07.2026.
  - README archiwum na HF odsyła do LICENSE w przestrzeni oryginalnej (Supertone/supertonic-3), nie archiwalnej. Treść jest identyczna, więc to tylko uwaga formalna.
  - Pochodzenie barw F1–M5 i dane treningowe: źródło ich nie ujawnia. Karta słusznie zostawia to do potwierdzenia (C5.6), ale u źródła nie ma czego potwierdzić — informację można uzyskać tylko od Supertone.
- **Dowody:**
  - https://huggingface.co/api/models/supertone-oss-archive/supertonic-3 — „metadane: license: openrail; sha: aafc6e32416a594460b32413efc49d7fe4ce6d46; createdAt 2026-09-08”
  - https://huggingface.co/supertone-oss-archive/supertonic-3/blob/aafc6e32416a594460b32413efc49d7fe4ce6d46/LICENSE — „tytuł pliku: BigScience Open RAIL-M License, dated August 18, 2022 (parafraza treści: Załącznik A pkt a–m, pkt 4a–4d, 5, 7)”
  - https://huggingface.co/supertone-oss-archive/supertonic-3/blob/aafc6e32416a594460b32413efc49d7fe4ce6d46/README.md — „parafraza sekcji License: kod przykładowy na MIT, model na OpenRAIL-M; nota Copyright (c) 2026 Supertone Inc.”
  - https://github.com/supertone-oss-archive/supertonic/blob/main/LICENSE — „nota: MIT License, Copyright (c) 2025 Supertone Inc.”
  - https://api.github.com/repos/supertone-inc/supertonic — „przekierowanie na supertone-oss-archive/supertonic; archived: true; spdx_id: MIT; pushed_at 2026-09-09T06:54:13Z”
  - https://github.com/supertone-oss-archive/supertonic/blob/main/README.md — „parafraza: repozytorium zarchiwizowane, rozwój i wsparcie zakończone; wagi przeniesione do przestrzeni HF supertone-oss-archive; zapowiedź z 23.07.2026; Voice Builder niedostępny po 31.08.2026”
- **Uwaga krytyka:** (1) Brak pola card_license. (2) Wynik pomija, że Alfa nie instaluje tekstu licencji Open RAIL-M obok modelu. Lista FILES w crates/app-models/src/supertonic.rs (16 plików: onnx i voice_styles) nie zawiera pliku LICENSE, a licencja (pkt 4b) i notatka karty wymagają przekazania kopii licencji. Wynik odnotował ten brak dla Smart Turn, dla Supertonic już nie. (3) W polu watermark obowiązek oznaczania zawęża się do treści rozpowszechnianych. Załącznik A pkt e obejmuje też generowanie treści i umieszczanie ich w dowolnym kontekście bez jasnej informacji, że wygenerowała je maszyna. (4) matches_card=true mimo niepoprawnego wyrażenia SPDX, które wynik sam wskazuje w rozbieżnościach.

### `piper-pl_PL-gosia-medium`

- **Karta w programie:** MIT (MIT (repozytorium piper-voices); licencja zbioru nagrań głosu — według MODEL_CARD), komercyjnie: unknown, wskazanie autora: True.
- **Źródło:** https://huggingface.co/rhasspy/piper-voices/tree/v1.0.0/pl/pl_PL/gosia/medium (tag v1.0.0 = commit 375a0fe6…)
- **Licencja u źródła:** Repozytorium piper-voices: MIT tylko w metadanych HF (w drzewie v1.0.0 nie ma pliku LICENSE ani nazwy właściciela praw). Zbiór nagrań „gosia”: CC0 (MODEL_CARD i README OHF-Voice/voice-datasets). Model bazowy: głos en_US „lessac” (medium), wytrenowany na zbiorze Blizzard Challenge 2013 Lessac z osobną licencją CSTR / Lessac Technologies. Tej licencji nie udało się przeczytać u źródła.
- **Tekst licencji:** https://huggingface.co/rhasspy/piper-voices/blob/v1.0.0/pl/pl_PL/gosia/medium/MODEL_CARD ; zbiór: https://github.com/OHF-Voice/voice-datasets (README) ; model bazowy: https://huggingface.co/rhasspy/piper-voices/blob/v1.0.0/en/en_US/lessac/medium/MODEL_CARD → https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html
- **Wydawca:** Zbiór nagrań: Nabu Casa / Open Home Foundation (repozytorium OHF-Voice/voice-datasets, dawniej NabuCasa/voice-datasets; stary adres przekierowuje). Repozytorium głosów na HF: rhasspy (projekt Piper). MODEL_CARD nie podaje, kto wytrenował głos ani kim jest mówczyni.
- **Ograniczenia i warunki:**
  - Głos jest dostrojeniem checkpointu en_US „lessac” medium, wytrenowanego na zbiorze Blizzard 2013 Lessac. Według opisu strony CSTR w wynikach wyszukiwarki zbiór jest dostępny tylko do użytku niekomercyjnego, na licencji akceptowanej przy rejestracji. Samej strony licencji nie dało się otworzyć (serwer CSTR odrzucał połączenia 2026-10-21), więc to ograniczenie nie jest potwierdzone u źródła.
  - W dyskusji #271 w repozytorium autora Pipera opiekun pisze, że Piper nie nakłada na modele głosów dodatkowych licencji, a ocena należy do użytkownika. Inny uczestnik (nie opiekun) czyta licencję Blizzard jako wykluczającą komercyjne użycie pochodnych, czyli także głosów dostrojonych z lessac.
  - README Pipera ostrzega, że niektóre głosy mogą mieć restrykcyjne licencje, i odsyła do MODEL_CARD.
  - CC0 zbioru dotyczy praw autorskich do nagrań. Źródło nie mówi nic o zgodzie mówczyni ani o prawach do jej głosu i wizerunku; zbiór opisano tylko jako crowdsourcing dla Year of Voice Home Assistanta.
  - README zbioru prosi o cytowanie repozytorium. To prośba, nie warunek CC0.
- **Znak wodny:** Brak. Piper nie dodaje znaku wodnego (brak wzmianki w README i 108 plikach tekstowych i źródłowych tagu 2023.11.14-2). MODEL_CARD głosu też nic o nim nie mówi.
- **Rozbieżności z kartą:**
  - Karta zostawia licencję nagrań do potwierdzenia. U źródła da się ją już ustalić: MODEL_CARD podaje CC0, zgodnie z README OHF-Voice/voice-datasets (pl_PL gosia dodany 2023-05-25).
  - Karta pomija model bazowy. MODEL_CARD mówi, że głos dostrojono z en_US „lessac” (medium), a lessac trenowano na Blizzard 2013 Lessac z osobną licencją (według wyszukiwarki tylko niekomercyjną). To główna niepewność co do użycia komercyjnego, a karta jej nie wymienia.
  - Pole id „MIT” odnosi się wyłącznie do metadanych HF repozytorium piper-voices. W repozytorium nie ma pliku LICENSE z właścicielem praw, a sam MODEL_CARD nie podaje MIT.
  - model-cards.json twierdzi, że zgoda lektorki jest w karcie głosu u źródła. MODEL_CARD nie zawiera żadnej informacji o zgodzie ani o mówczyni.
  - Adres zbioru w MODEL_CARD v1.0.0 (NabuCasa/voice-datasets) jest nieaktualny: przekierowuje na OHF-Voice/voice-datasets (tak podaje gałąź main).
  - Pole author karty („Projekt Piper”) pomija wydawcę zbioru nagrań: Nabu Casa / Open Home Foundation.
- **Dowody:**
  - https://huggingface.co/rhasspy/piper-voices/blob/v1.0.0/pl/pl_PL/gosia/medium/MODEL_CARD — „pole: License: CC0; Dataset URL: https://github.com/NabuCasa/voice-datasets; Training: Finetuned from U.S. English lessac voice (medium quality)”
  - https://huggingface.co/rhasspy/piper-voices/blob/v1.0.0/en/en_US/lessac/medium/MODEL_CARD — „pole: License: https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html; Trained from scratch”
  - https://github.com/OHF-Voice/voice-datasets — „parafraza README: zbiory zebrane crowdsourcingiem dla Year of Voice (Nabu Casa), licencja CC0, prośba o cytowanie; wiersz pl_PL gosia, 2023-05-25”
  - https://huggingface.co/api/models/rhasspy/piper-voices — „metadane: license: mit (w drzewie v1.0.0 brak pliku LICENSE)”
  - https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/ — „strona niedostępna (ECONNREFUSED); opis z wyszukiwarki (źródło pośrednie): dane wyłącznie do użytku niekomercyjnego, po akceptacji licencji”
  - https://github.com/rhasspy/piper/discussions/271 — „parafraza: opiekun nie nakłada licencji na głosy, ocena po stronie użytkownika; uczestnik (nie opiekun) uważa, że licencja Blizzard wyklucza komercyjne użycie pochodnych”
- **Uwaga krytyka:** (1) Brak pola card_license. (2) Rozbieżność z model-cards.json jest przedstawiona za mocno. Wpis mówi, że pochodzenie nagrań i zgoda lektorki są w karcie głosu u źródła, ale dodaje zastrzeżenie, że to do potwierdzenia, czyli nie twierdzi tego kategorycznie. Ta rozbieżność dotyczy zresztą model-cards.json, a nie license-cards.json. Samo ustalenie, że MODEL_CARD nie mówi nic o zgodzie mówczyni, jest prawdziwe (sprawdzone). (3) Strony licencji Lessac (CSTR) również nie udało się otworzyć (ECONNREFUSED, 21.10.2026). Ograniczenie niekomercyjne pozostaje niepotwierdzone, tak jak zaznaczył wynik.

### `sidecar-piper`

- **Karta w programie:** MIT AND GPL-3.0-or-later (MIT (Piper 2023.11.14-2); GPL-3.0 (espeak-ng w archiwum)), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://github.com/rhasspy/piper/releases/tag/2023.11.14-2 (zasób piper_windows_amd64.zip, 22 477 236 B; nie pobierano)
- **Licencja u źródła:** Piper: MIT (LICENSE.md, Copyright (c) 2022 Michael Hansen). Według reguł instalacji w CMakeLists tagu paczka zawiera też komponenty z piper-phonemize: bibliotekę espeak-ng i espeak-ng-data (fork rhasspy/espeak-ng, GPL-3.0; upstream: GPL w wersji 3 lub późniejszej), piper_phonemize (MIT, Michael Hansen), onnxruntime (domyślnie 1.14.1) i libtashkeel_model.ort. Licencji onnxruntime i libtashkeel tu nie sprawdzano.
- **Tekst licencji:** https://github.com/rhasspy/piper/blob/2023.11.14-2/LICENSE.md ; https://github.com/espeak-ng/espeak-ng/blob/master/COPYING ; https://github.com/rhasspy/piper-phonemize/blob/master/LICENSE.md
- **Wydawca:** Michael Hansen (rhasspy). Repozytorium zarchiwizowane; rozwój przeniesiono do OHF-Voice/piper1-gpl (GPL-3.0, Open Home Foundation). espeak-ng to projekt espeak-ng.
- **Ograniczenia i warunki:**
  - Rozpowszechnianie paczki z espeak-ng wymaga spełnienia warunków GPL-3.0: tekst licencji, dostęp do kodu źródłowego, w tym konkretnego forka rhasspy/espeak-ng (commit 0f65aa301e0d6bae5e172cc74197d32a6182200f).
  - README Pipera mówi, że Piper służy do badań nad syntezą mowy i nie nakłada ograniczeń na głosy, ale głosy mają własne licencje (MODEL_CARD).
  - Opiekun Pipera (dyskusja #271) przyznaje, że nie wie, czy GPL fonemizera dotyczy generowanych plików WAV.
  - Repozytorium zarchiwizowane: wydanie z 2023-11-14 nie dostanie poprawek ani łatek bezpieczeństwa.
- **Znak wodny:** Brak. Piper 2023.11.14-2 nie dodaje znaku wodnego i go nie wymaga: nie ma wzmianki w README ani w 108 plikach tekstowych i źródłowych tagu (przeszukane 2026-10-21).
- **Rozbieżności z kartą:**
  - Katalog (crates/app-models/src/data.rs, pole licencji pozycji) podaje samo „MIT”, a karta: „MIT AND GPL-3.0-or-later”. Karta jest bliższa stanowi u źródła; wpis katalogu pomija GPL espeak-ng.
  - Karta nie wymienia pozostałych składników paczki: piper_phonemize (MIT), onnxruntime (pobierany przez piper-phonemize) i libtashkeel_model.ort.
  - Paczka zawiera fork rhasspy/espeak-ng (GitHub API: GPL-3.0, repozytorium zarchiwizowane), a nie upstream espeak-ng. Upstream deklaruje GPL w wersji 3 lub późniejszej, więc „-or-later” w karcie wynika z upstreamu; w forku tego nie sprawdzano.
  - Zawartość archiwum zip nie została sprawdzona (bez pobierania). Skład paczki wynika z reguł install() w CMakeLists tagu 2023.11.14-2.
  - Data archiwizacji rhasspy/piper nie jest podana u źródła (ostatni push: 2025-08-26). ADR 0024 podaje 2025-10 ze źródła wtórnego.
- **Dowody:**
  - https://github.com/rhasspy/piper/blob/2023.11.14-2/LICENSE.md — „nota: MIT License, Copyright (c) 2022 Michael Hansen”
  - https://api.github.com/repos/rhasspy/piper — „archived: true; spdx_id: MIT; pushed_at 2025-08-26T15:01:29Z”
  - https://github.com/rhasspy/piper/blob/master/README.md — „parafraza: rozwój przeniesiony do https://github.com/OHF-Voice/piper1-gpl”
  - https://github.com/rhasspy/piper/blob/2023.11.14-2/README.md — „parafraza: Piper do badań nad TTS, bez dodatkowych ograniczeń dla głosów; niektóre głosy mają restrykcyjne licencje — sprawdzić MODEL_CARD”
  - https://github.com/rhasspy/piper/blob/2023.11.14-2/CMakeLists.txt — „parafraza: install() kopiuje DLL z piper-phonemize, katalog espeak-ng-data i libtashkeel_model.ort”
  - https://github.com/rhasspy/piper-phonemize/blob/master/CMakeLists.txt — „parafraza: espeak-ng z archiwum rhasspy/espeak-ng, commit 0f65aa30…; ONNXRUNTIME_VERSION 1.14.1”
  - https://github.com/espeak-ng/espeak-ng/blob/master/README.md — „parafraza sekcji License Information: GPL w wersji 3 lub późniejszej”
- **Uwaga krytyka:** Błąd faktyczny w ograniczeniach i dowodach: wynik twierdzi, że paczka zawiera fork rhasspy/espeak-ng przypięty do commitu 0f65aa301e0d…, i opiera to na bieżącym master piper-phonemize. Wydanie 2023.11.14-2 zbudowano 14.11.2023, a zasób piper_windows_amd64.zip wgrano 14.11.2023 o 19:09Z. CMakeLists.txt Pipera przy tagu pobiera piper-phonemize z refs/heads/master. W tamtym dniu piper-phonemize (commit fccd4f335a z 14.11.2023) pobierał rhasspy/espeak-ng z refs/heads/master.zip. Przypięcie do 0f65aa30 dodał dopiero commit bfc2e75499 z 5.12.2023, już po wydaniu. Dokładna wersja espeak-ng w paczce jest więc nieustalona, a od niej zależy, jakie źródła trzeba udostępnić zgodnie z GPL-3.0.
- **Uwaga krytyka:** Brak pola card_license (karta: MIT AND GPL-3.0-or-later, url do LICENSE.md na gałęzi master, commercial conditional, attribution true, confirmed false). Wynik linkuje LICENSE.md przy tagu, ale nie odnotowuje, że karta wskazuje gałąź master.

### `sidecar-pocket-tts`

- **Karta w programie:** CC-BY-4.0 (CC BY 4.0 (model Pocket TTS); wrapper — kod Alfy), komercyjnie: unknown, wskazanie autora: True.
- **Źródło:** Model bazowy: https://huggingface.co/kyutai/pocket-tts ; kod: https://github.com/kyutai-labs/pocket-tts. Wariant PL: katalog Alfy nie podaje żadnego adresu. Jedyny znaleziony kandydat: https://huggingface.co/shefowl/pocket-tts-polish-6l (rewizja a8630f2a39055d3e5a91acb7922e31bd0c506cfe).
- **Licencja u źródła:** Kod Pocket TTS (Kyutai): MIT. Wagi kyutai/pocket-tts: CC-BY-4.0, dostęp po akceptacji warunków (gated) z listą zakazanych zastosowań. Polski model społeczności shefowl/pocket-tts-polish-6l: CC-BY-4.0, wydawca deklaruje ją jako odziedziczoną po kyutai/pocket-tts (to fine-tune z destylacją). Wrapper JSON-lines to kod Alfy.
- **Tekst licencji:** https://github.com/kyutai-labs/pocket-tts/blob/main/LICENSE ; https://huggingface.co/kyutai/pocket-tts ; https://huggingface.co/shefowl/pocket-tts-polish-6l/blob/a8630f2a39055d3e5a91acb7922e31bd0c506cfe/README.md (sekcja License and attribution)
- **Wydawca:** Kyutai (model bazowy i kod). Wariant PL: użytkownik HF „shefowl”, niezależny od Kyutai. Kyutai tylko wymienia ten model w README, w sekcji modeli wytrenowanych przez społeczność; nie jest to oficjalne wydanie autora.
- **Ograniczenia i warunki:**
  - Warunki dostępu Kyutai (gated) zakazują m.in. podszywania się pod głos lub klonowania go bez zgody, dezinformacji, oszukańczych połączeń i przedstawiania wygenerowanego dźwięku jako prawdziwych nagrań osób i zdarzeń. Karta modelu PL rozciąga tę prośbę na siebie.
  - CC-BY-4.0 wymaga podania autorów: Kyutai, a dla wariantu PL także shefowl.
  - Korpus treningowy modelu PL (425,1 h): audiobooki z domeny publicznej, CML-TTS Polish (CC-BY-4.0), niepodpisany jednomówczy zbiór studyjny, nagrania badawcze PWR AZON, uzupełnienie liczbowe i mowa konwersacyjna. Licencje poszczególnych źródeł (poza CML-TTS) nie są podane. Licencję AZON (CC BY-SA 4.0) zna tylko źródło pośrednie (karty zbioru BIGOS) i nie jest potwierdzona.
  - Gotowe głosy Kyutai (kyutai/tts-voices) mają mieszane licencje: CC0, CC BY 4.0 i CC BY-NC 4.0 (expresso, ears — tylko niekomercyjnie). Ma to znaczenie, jeśli Alfa użyje któregoś z nich jako próbki.
  - Model PL nie ma wbudowanych mówczyń: klonuje głos z próbki. Dołączona próbka reference.wav pochodzi z CML-TTS (CC-BY-4.0).
- **Znak wodny:** Brak. Pocket TTS nie dodaje znaku wodnego i go nie wymaga: nie ma wzmianki w README, karcie HF ani w 152 plikach tekstowych i źródłowych repozytorium kyutai-labs/pocket-tts (gałąź main, 2026-10-21). Karta modelu PL też o nim nie wspomina. Hipoteza o znaku wodnym w Pocket TTS od Kyutai nie znajduje potwierdzenia u źródła.
- **Rozbieżności z kartą:**
  - (b) Pochodzenie: katalog Alfy nie podaje dla tej pozycji żadnego adresu pobrania, pliku ani rewizji. Lista plików jest pusta, instalacja ręczna, a pole source wskazuje lokalny crates/voice-tts-impl/README.md, który polskiego modelu nie nazywa. Z repozytorium nie da się więc ustalić, którego modelu wpis używa, ani czy to oficjalne wydanie. Oficjalnym wydaniem Kyutai na pewno nie jest, bo Kyutai nie wydaje modelu polskiego.
  - Jedyny polski model Pocket TTS znaleziony na HF (wyszukiwanie przez API HF): shefowl/pocket-tts-polish-6l, utworzony 2026-09-17, wymieniony w oficjalnym README Kyutai jako model społeczności. Za powiązaniem przemawia poszlaka: RTF ≈ 0,21 z dokumentów Alfy (ADR 0004, 0011, PLAN) zgadza się z RTF 0.21 na procesorze stacjonarnym z karty tego modelu; to jednak nie dowód. ADR 0011 (2026-09-30) powstał po publikacji modelu.
  - Głosów pl-f1 i pl-f2 nie ma w żadnym znalezionym źródle. Model PL nie ma wbudowanych mówczyń i ma tylko jedną próbkę reference.wav (CML-TTS). Pochodzenie próbek pl-f1 i pl-f2 jest nieustalone.
  - Pole url karty wskazuje ogólną stronę CC BY 4.0 zamiast źródła (kyutai/pocket-tts lub repozytorium modelu PL z rewizją).
  - Karta nie wspomina, że kod Kyutai jest na MIT i że wagi bazowe są dostępne po akceptacji warunków (gated) z listą zakazów.
  - Kyutai wydaje teraz oficjalny program pocket-tts dla Windows (x86) bez Pythona: port w Rust na xn-ptts, czyta też modele społeczności. Licencji xn-ptts tu nie sprawdzano. Własny wrapper Alfy (ADR 11) może więc być zbędny.
  - Dokumenty Alfy (ADR 0024, docs/plan/15) mówią o 6 językach. README Kyutai wymienia dziś 7 (doszedł niderlandzki), metadane HF nadal 6. Polskiego oficjalnie brak, co potwierdzono.
  - Wariant PL ma CC-BY-4.0 według deklaracji wydawcy, ale licencje części danych treningowych są nieujawnione. Status komercyjny wariantu PL jest więc nieustalony, mimo że sama CC-BY-4.0 pozwala na użycie komercyjne.
- **Dowody:**
  - https://huggingface.co/kyutai/pocket-tts — „"voice impersonation or cloning without explicit and lawful consent" (warunki dostępu; metadane: license: cc-by-4.0; języki en, fr, de, pt, it, es)”
  - https://api.github.com/repos/kyutai-labs/pocket-tts — „license spdx_id: MIT; archived: false”
  - https://github.com/kyutai-labs/pocket-tts/blob/main/README.md — „parafraza: 7 języków bez polskiego; sekcja modeli społeczności zawiera Pocket TTS Polish (@shefowl, 6 warstw) z rewizją a8630f2a…; oficjalny program bez Pythona także dla Windows”
  - https://huggingface.co/api/models/shefowl/pocket-tts-polish-6l — „metadane: license: cc-by-4.0; base_model: kyutai/pocket-tts; base_model_relation: finetune; createdAt 2026-09-17”
  - https://huggingface.co/shefowl/pocket-tts-polish-6l/blob/a8630f2a39055d3e5a91acb7922e31bd0c506cfe/README.md — „parafraza: licencja odziedziczona po kyutai/pocket-tts; korpus 425,1 h z sześciu źródeł; reference.wav z CML-TTS (CC-BY-4.0); RTF 0.21 na procesorze stacjonarnym”
  - https://huggingface.co/kyutai/tts-voices — „parafraza: licencje głosów według folderów: CC0, CC BY 4.0, CC BY-NC 4.0 (expresso, ears)”
- **Uwaga krytyka:** Brak pola card_license. Merytorycznie wynik się potwierdza: kyutai/pocket-tts ma dostęp warunkowy, licencję CC-BY-4.0 i 6 języków w metadanych. README wymienia 7 języków i model społeczności shefowl/pocket-tts-polish-6l z rewizją a8630f2a. ADR 0004 i 0011 (oba z 30.09.2026) podają RTF ≈ 0,21, czyli po publikacji modelu PL 17.09.2026.

### `silero-vad`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://pypi.org/project/silero-vad/6.2.3/ (plik silero_vad-6.2.3-py3-none-any.whl z files.pythonhosted.org, wysłany 2026-09-23; repozytorium autora: https://github.com/snakers4/silero-vad)
- **Licencja u źródła:** MIT (plik LICENSE w repozytorium autora; w metadanych PyPI pole license jest puste, jest tylko klasyfikator MIT)
- **Tekst licencji:** https://github.com/snakers4/silero-vad/blob/master/LICENSE
- **Wydawca:** Silero Team (hello@silero.ai), repozytorium snakers4/silero-vad
- **Ograniczenia i warunki:**
  - MIT: w każdej kopii trzeba zachować notę copyright i tekst licencji. Plik LICENSE nie ma dodatkowych klauzul.
  - Danych treningowych nie wskazano z nazwy. README mówi tylko o dużych korpusach w ponad 6000 językach, więc ich licencji nie da się ustalić.
  - Plik używany przez Alfę (silero_vad_op18_ifless.onnx) jest w repozytorium autora w src/silero_vad/data (2 845 718 B), czyli to oficjalny wariant autora.
- **Znak wodny:** Nie dotyczy (detektor mowy, nie silnik TTS).
- **Rozbieżności z kartą:**
  - Brak rozbieżności merytorycznych z kartą.
  - Niespójność po stronie źródła (kosmetyczna): odznaka licencji w README ma tekst alternatywny „License: CC BY-NC 4.0”, ale prowadzi do pliku LICENSE z MIT, a tekst README mówi o MIT. Rozstrzyga plik LICENSE.
  - Na karcie można zapisać, że w metadanych PyPI 6.2.3 licencja jest podana tylko klasyfikatorem (pola license i license_expression są puste).
- **Dowody:**
  - https://raw.githubusercontent.com/snakers4/silero-vad/master/LICENSE — „"Copyright (c) 2020-present Silero Team"”
  - https://pypi.org/pypi/silero-vad/6.2.3/json — „"License :: OSI Approved :: MIT License"”
  - https://raw.githubusercontent.com/snakers4/silero-vad/master/README.md — „"Published under permissive license (MIT)"”
  - https://raw.githubusercontent.com/snakers4/silero-vad/master/README.md — „"[![License: CC BY-NC 4.0]" (tekst alternatywny odznaki prowadzącej do pliku LICENSE z MIT)”
  - https://api.github.com/repos/snakers4/silero-vad/contents/src/silero_vad/data — „silero_vad_op18_ifless.onnx (lista plików katalogu, bez cytatu)”

### `openwakeword-features`

- **Karta w programie:** Apache-2.0 (Apache License 2.0 (kod openWakeWord); licencja modeli cech — do potwierdzenia), komercyjnie: unknown, wskazanie autora: True.
- **Źródło:** https://pypi.org/project/openwakeword/0.5.1/ (wheel openwakeword-0.5.1-py3-none-any.whl, wysłany 2023-09-02; Alfa wyjmuje z niego tylko resources/models/melspectrogram.onnx i embedding_model.onnx; repozytorium: https://github.com/dscripka/openWakeWord)
- **Licencja u źródła:** Kod: Apache-2.0 (LICENSE i klasyfikator PyPI). Modele: README (to samo brzmienie w 0.5.1 i na main) obejmuje „wszystkie dołączone wytrenowane modele” licencją CC-BY-NC-SA-4.0 i nie wyłącza z niej modeli cech. Pierwowzór embeddingu, Google speech_embedding, jest u Google na Apache 2.0. Licencja samych plików melspectrogram.onnx i embedding_model.onnx jest u źródła NIEROZSTRZYGNIĘTA.
- **Tekst licencji:** https://github.com/dscripka/openWakeWord/blob/main/LICENSE (kod); sekcja License w README: https://github.com/dscripka/openWakeWord#license (modele)
- **Wydawca:** David Scripka (openWakeWord). Pierwowzór embeddingu: Google (speech_embedding, TFHub, dziś Kaggle Models; autorzy pracy: Lin, Kilgour, Roblek, Sharifi).
- **Ograniczenia i warunki:**
  - Jeśli pliki cech podlegają ogólnemu zdaniu z README: CC BY-NC-SA 4.0 zabrania użycia komercyjnego (NC) i wymaga tej samej licencji dla utworów zależnych (SA).
  - README tłumaczy licencję NC danymi treningowymi o „nieznanej lub restrykcyjnej” licencji, bez wskazania zbiorów.
  - Embedding to według README ręczna reimplementacja modelu Google. Model Google wytrenowano na około 200 mln 2-sekundowych klipów z YouTube (arXiv 2002.01322). Prawa do tych nagrań nie są nigdzie omówione.
  - melspectrogram.onnx to według README implementacja ONNX funkcji melspectrogram z Torcha o stałych parametrach. Licencji dla tego pliku osobno nie podano.
  - W repozytorium są otwarte zgłoszenia #313, #338, #346, #348 i #357 (od 2026-01 do 2026-10-21) z pytaniem właśnie o licencję tych dwóch plików do użytku komercyjnego. Na stronach zgłoszeń nie widać odpowiedzi autora (strony oglądane bez logowania).
  - Wheel zawiera też gotowe klasyfikatory (alexa, hey jarvis itd.), które wprost są na CC BY-NC-SA 4.0. Alfa ich nie wyjmuje.
- **Znak wodny:** Nie dotyczy (wykrywanie słowa wywoławczego, nie TTS).
- **Rozbieżności z kartą:**
  - Karta dobrze opisuje stan sprawy (commercial=unknown, zastrzeżenie w notes). Jednak pole id = „Apache-2.0” dotyczy tylko kodu. Dla dwóch plików ONNX, których Alfa naprawdę używa, autor nie podał licencji, a ogólne zdanie z README wskazuje na CC-BY-NC-SA-4.0. Samo SPDX może więc wprowadzać w błąd automatyczne sprawdzenia i interfejs.
  - Karta nie odnotowuje otwartych zgłoszeń #313/#338/#346/#348/#357 ani tego, że autor nie odpowiedział. To najpewniejszy dowód, że sprawa jest u źródła nierozstrzygnięta.
  - Licencja Apache 2.0 pierwowzoru Google jest potwierdzona w API Kaggle (licenseName „Apache 2.0”). Strona TFHub przekierowuje na Kaggle, a zwykła strona Kaggle nie wyświetliła treści.
- **Dowody:**
  - https://raw.githubusercontent.com/dscripka/openWakeWord/main/LICENSE — „"Apache License Version 2.0, January 2004"”
  - https://raw.githubusercontent.com/dscripka/openWakeWord/main/README.md — „"All of the included pre-trained models are licensed under the Creative Commons"”
  - https://raw.githubusercontent.com/dscripka/openWakeWord/main/README.md — „"due to the inclusion of datasets with unknown or restrictive licensing"”
  - https://pypi.org/pypi/openwakeword/0.5.1/json — „"provided by Google as a TFHub module under an Apache-2.0 license"”
  - https://pypi.org/pypi/openwakeword/0.5.1/json — „"License :: OSI Approved :: Apache Software License"”
  - https://www.kaggle.com/api/v1/models/google/speech-embedding/get — „"licenseName": "Apache 2.0"”
  - https://ar5iv.labs.arxiv.org/html/2002.01322 — „"We used roughly 200 million 2-second audio clips from YouTube" (ar5iv: wersja HTML pracy z arXiv)”
  - https://github.com/dscripka/openWakeWord/issues/348 — „Zgłoszenie otwarte, brak odpowiedzi autora (bez cytatu)”
  - https://github.com/dscripka/openWakeWord/issues/338 — „Zgłoszenie otwarte, brak odpowiedzi autora (bez cytatu)”

### `wespeaker-resnet34`

- **Karta w programie:** Apache-2.0 (Apache License 2.0 (WeSpeaker); warunki zbioru VoxCeleb — do sprawdzenia), komercyjnie: unknown, wskazanie autora: True.
- **Źródło:** https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models (plik wespeaker_en_voxceleb_resnet34.onnx, 26 534 365 B, wysłany 2024-10-14, GitHub nie podaje sumy). Pochodzenie: skrypt sherpa-onnx scripts/wespeaker/run.sh klonuje HF openspeech/wespeaker-models (serwis pośredni: kopia plików z WeSpeaker), zmienia nazwę voxceleb_resnet34.onnx i dodaje metadane. Oficjalny model: https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34
- **Licencja u źródła:** Kod WeSpeaker: Apache-2.0. Kod sherpa-onnx: Apache-2.0. MODEL: dokumentacja WeSpeaker (docs/pretrained.md) mówi, że model VoxCeleb ma licencję zbioru, czyli CC-BY-4.0. Karta HF Wespeaker/wespeaker-voxceleb-resnet34 ma znacznik apache-2.0, a jej treść jest pusta, więc źródło jest wewnętrznie sprzeczne. Wydanie sherpa-onnx odsyła do licencji każdego modelu osobno.
- **Tekst licencji:** https://github.com/wenet-e2e/wespeaker/blob/master/docs/pretrained.md (licencja modeli); https://github.com/wenet-e2e/wespeaker/blob/master/LICENSE (kod); https://mm.kaist.ac.kr/datasets/voxceleb/ (zbiór)
- **Wydawca:** Projekt WeSpeaker (wenet-e2e). Plik ONNX z metadanymi: k2-fsa/sherpa-onnx. Zbiór VoxCeleb: A. Nagrani, J. S. Chung, A. Zisserman (Oxford VGG; strona zbioru, do której linkuje WeSpeaker, jest na mm.kaist.ac.kr).
- **Ograniczenia i warunki:**
  - Strona VoxCeleb, do której odsyła WeSpeaker: zbiór na CC BY 4.0, ale udostępniany do pobrania „for research purposes”. Użycia komercyjnego strona wprost nie omawia.
  - Prawa autorskie do nagrań zostają przy właścicielach filmów z YouTube („The copyright remains with the original owners of the video”).
  - Strony Oxford VGG (vox1/vox2) podają CC BY-SA 4.0 tylko dla metadanych (adresy URL i znaczniki czasu). Pliki audio nie są już tam udostępniane. Do spraw prywatności strony odsyłają do „Dataset Privacy Notice”.
  - CC BY 4.0, jeśli dotyczy modelu, wymaga wskazania autora i zaznaczenia zmian. sherpa-onnx zmienił nazwę pliku i dodał metadane.
  - Poza licencją: odcisk głosu to dane biometryczne. Karta modelu Alfy już to opisuje.
- **Znak wodny:** Nie dotyczy (rozpoznawanie mówcy, nie TTS).
- **Rozbieżności z kartą:**
  - Karta (notes) podaje „Kod i model na Apache-2.0”. Tymczasem dokumentacja WeSpeaker mówi, że wytrenowany model VoxCeleb podlega CC BY 4.0, czyli licencji zbioru. Apache-2.0 dla modelu wynika tylko ze znacznika na pustej karcie HF. Źródło jest sprzeczne i nie da się rozstrzygnąć, która licencja obowiązuje.
  - Pole id = „Apache-2.0” nie oddaje licencji modelu. Lepiej opisać to jako Apache-2.0 (kod) oraz CC-BY-4.0 (model według WeSpeaker), z warunkami VoxCeleb do potwierdzenia.
  - Szablon manifestu w crates/app-models/src/data_speech.rs zapisuje "license": "Apache-2.0" w pliku wespeaker-resnet34.speaker.json, więc to uproszczenie trafia także do danych instalacji.
  - Karta przypisuje VoxCeleb Uniwersytetowi Oksfordzkiemu. Strona zbioru wskazywana przez WeSpeaker (mm.kaist.ac.kr) ogranicza pobieranie do celów badawczych („for research purposes”), czego karta nie odnotowuje.
  - Plik pochodzi z serwisu pośredniego (HF openspeech/wespeaker-models), a nie bezpośrednio od WeSpeaker. Pozycja w katalogu nie ma przypiętej sumy SHA-256, a GitHub jej nie publikuje.
- **Dowody:**
  - https://raw.githubusercontent.com/wenet-e2e/wespeaker/master/docs/pretrained.md — „"The pretrained model in WeNet follows the license of it's corresponding dataset."”
  - https://raw.githubusercontent.com/wenet-e2e/wespeaker/master/docs/pretrained.md — „"the pretrained model on VoxCeleb follows Creative Commons Attribution 4.0 International License"”
  - https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34 — „"License: apache-2.0" (karta modelu bez treści)”
  - https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models — „"Each model has its own license."”
  - https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/scripts/wespeaker/run.sh — „"mv voxceleb_resnet34.onnx wespeaker_en_voxceleb_resnet34.onnx"”
  - https://huggingface.co/openspeech/wespeaker-models — „"Files in this repo are downloade from" (strona modeli WeSpeaker; serwis pośredni)”
  - https://mm.kaist.ac.kr/datasets/voxceleb/ — „"available to download for research purposes under a Creative Commons Attribution 4.0"”
  - https://mm.kaist.ac.kr/datasets/voxceleb/ — „"The copyright remains with the original owners of the video."”
  - https://www.robots.ox.ac.uk/~vgg/data/voxceleb/vox2.html — „"licensed under a Creative Commons Attribution-ShareAlike 4.0 International License" (dotyczy metadanych)”
  - https://raw.githubusercontent.com/wenet-e2e/wespeaker/master/LICENSE — „"Apache License Version 2.0, January 2004"”
- **Uwaga krytyka:** Słaba rozbieżność. Uwaga, że karta przypisuje VoxCeleb Uniwersytetowi Oksfordzkiemu, nie jest błędem karty, bo VoxCeleb pochodzi z Oxford VGG, co wynik sam przyznaje w polu publisher. Strona mm.kaist.ac.kr to obecne laboratorium współautora. Nową informacją jest tylko udostępnianie zbioru do celów badawczych (potwierdzone). Ustalenia o CC BY 4.0 dla modelu (docs/pretrained.md) i Apache-2.0 na pustej karcie HF się potwierdzają.

### `sidecar-whisper-cpu`

- **Karta w programie:** MIT (MIT License), komercyjnie: yes, wskazanie autora: True.
- **Źródło:** https://github.com/ggml-org/whisper.cpp/releases/tag/v1.8.1 (plik whisper-bin-x64.zip, 3 831 744 B, sha256 cc5d6126e025ef463524ed74c94d4b6a40bb67c2d1c3cb5aca02c773c388bdad, wydanie z 2025-10-12, zbudowane przez github-actions[bot])
- **Licencja u źródła:** MIT (whisper.cpp i ggml, „The ggml authors”). Archiwum zawiera też SDL2.dll (SDL 2.28.5) na licencji zlib: według build.yml budowa idzie z -DWHISPER_SDL2=ON i kopiuje SDL2.dll do build/bin/Release.
- **Tekst licencji:** https://github.com/ggml-org/whisper.cpp/blob/v1.8.1/LICENSE; SDL2: https://github.com/libsdl-org/SDL/blob/release-2.28.5/LICENSE.txt
- **Wydawca:** ggml-org (Georgi Gerganov i współtwórcy, „The ggml authors”). Kompilację wydaje projekt w GitHub Actions.
- **Ograniczenia i warunki:**
  - MIT: przy rozpowszechnianiu trzeba dołączyć notę copyright i tekst licencji. Workflow nie wkłada do archiwum żadnych plików LICENSE, więc przy dalszej dystrybucji tekst MIT (oraz SDL/zlib) trzeba dołączyć samemu.
  - SDL2 (zlib): wzmianka w dokumentacji produktu jest „mile widziana, ale niewymagana”. Noty nie wolno usuwać z dystrybucji źródłowych.
  - Modele Whisper to osobne pozycje katalogu z osobną licencją.
- **Znak wodny:** Nie dotyczy (rozpoznawanie mowy, nie TTS).
- **Rozbieżności z kartą:**
  - Karta nie wspomina o SDL2.dll (zlib) w archiwum. Dokładniej byłoby „MIT AND Zlib”. Zawartość archiwum ustaliłem z workflow budowy, nie z listy plików archiwum, którego nie pobierałem.
  - Karta pisze, że pozycja czeka na sumę archiwum. GitHub publikuje dla tego pliku SHA-256: cc5d6126e025ef463524ed74c94d4b6a40bb67c2d1c3cb5aca02c773c388bdad.
  - Poza kartą: katalog (data.rs) podaje rozmiar 8 MiB, a plik ma 3 831 744 B (ok. 3,65 MiB).
  - Copyright u źródła brzmi „The ggml authors”. Karta pisze „Georgi Gerganov i współtwórcy”. Treść ta sama, inne sformułowanie.
- **Dowody:**
  - https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.1/LICENSE — „"Copyright (c) 2023-2024 The ggml authors" (MIT License)”
  - https://api.github.com/repos/ggml-org/whisper.cpp/releases/tags/v1.8.1 — „whisper-bin-x64.zip, 3831744 B, sha256:cc5d6126… (dane z API, bez cytatu)”
  - https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.1/.github/workflows/build.yml — „"-DWHISPER_SDL2=ON"”
  - https://raw.githubusercontent.com/libsdl-org/SDL/release-2.28.5/LICENSE.txt — „"Copyright (C) 1997-2023 Sam Lantinga"”
- **Uwaga krytyka:** Brak istotnych błędów. Rozmiar 3 831 744 B, SHA-256 cc5d6126…, -DWHISPER_SDL2=ON, SDL 2.28.5 i 8 MiB w data.rs (FileSpec::new liczy w MiB) się potwierdzają. Drobiazg: test crates/app-models/tests/layout.rs już modeluje Release/SDL2.dll, więc obecność SDL2 w archiwum jest znana repozytorium.

### `sidecar-whisper-cuda`

- **Karta w programie:** MIT AND LicenseRef-NVIDIA-CUDA-EULA (MIT (whisper.cpp); NVIDIA CUDA Toolkit EULA (cudart, cuBLAS)), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://github.com/ggml-org/whisper.cpp/releases/tag/v1.8.1 (plik whisper-cublas-12.4.0-bin-x64.zip, 450 211 367 B, sha256 66638830959008de552de41223e718351abcf4e1b5c59188d1ae04ec20047c6b, wydanie z 2025-10-12, github-actions[bot])
- **Licencja u źródła:** MIT (whisper.cpp/ggml) dla programu, licencja NVIDIA CUDA Toolkit EULA (wersja 12.4) dla bibliotek DLL z CUDA, zlib dla SDL2.dll. Według build.yml krok „Copy CUDA DLLs” kopiuje WSZYSTKIE pliki *.dll z $env:CUDA_PATH\bin. Zainstalowane składniki: cuda_cudart, cuda_nvcc, cuda_nvrtc, libcublas, cuda_nvtx, cuda_profiler_api, visual_studio_integration, cuda_nvprof, cuda_cccl.
- **Tekst licencji:** https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html (EULA v12.4 dla tego toolkitu); https://github.com/ggml-org/whisper.cpp/blob/v1.8.1/LICENSE
- **Wydawca:** ggml-org (kompilacja w GitHub Actions); biblioteki CUDA: NVIDIA Corporation; SDL2: Sam Lantinga / libsdl-org
- **Ograniczenia i warunki:**
  - Załącznik A EULA v12.4 dopuszcza dystrybucję z aplikacją m.in. plików cudart.dll, cublas.dll, cublasLt.dll, nvrtc.dll, nvrtc-builtins.dll i nvToolsExt.dll.
  - Warunki z sekcji 1.1.2 EULA: aplikacja musi mieć „material additional functionality” ponad SDK, tylko ona może korzystać z tych bibliotek, warunki dla użytkowników muszą chronić NVIDIA co najmniej tak jak EULA, trzeba zgłaszać NVIDIA znane naruszenia i egzekwować własne umowy.
  - Składniki nvcc/ptxas i nvprof nie występują w Załączniku A. Workflow kopiuje wszystkie DLL z katalogu bin, więc nie sprawdziłem, czy w archiwum nie ma bibliotek spoza listy dozwolonych. Wymagałoby to listy plików archiwum (430 MB), którego nie pobierałem.
  - Archiwum nie zawiera tekstów licencji (workflow nie kopiuje plików LICENSE ani EULA).
  - Uwaga faktograficzna: dystrybucję przeprowadza ggml-org. Gdy Alfa pobiera archiwum z GitHuba na komputer użytkownika, sama go nie rozpowszechnia. Warunki EULA dotyczą Alfy, gdy dołączy te biblioteki do własnego pakietu.
- **Znak wodny:** Nie dotyczy (rozpoznawanie mowy, nie TTS).
- **Rozbieżności z kartą:**
  - Karta linkuje do bieżącej EULA (strona oznaczona v13.4, z 2026). Archiwum zbudowano z CUDA 12.4, więc dotyczy go EULA v12.4: https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html.
  - Karta wymienia tylko cudart i cuBLAS. Według build.yml do archiwum trafiają wszystkie DLL z katalogu bin zainstalowanych składników, więc prawdopodobnie także NVRTC (dozwolony) i ewentualnie inne. Nie sprawdzono na liście plików archiwum.
  - Karta nie wspomina o SDL2.dll (zlib).
  - Katalog nie ma przypiętej sumy (sha256: None). GitHub publikuje SHA-256 66638830959008de552de41223e718351abcf4e1b5c59188d1ae04ec20047c6b. Rozmiar 450 211 367 B (ok. 429 MiB) zgadza się z 430 MiB w katalogu.
- **Dowody:**
  - https://api.github.com/repos/ggml-org/whisper.cpp/releases/tags/v1.8.1 — „whisper-cublas-12.4.0-bin-x64.zip, 450211367 B, sha256:66638830… (dane z API, bez cytatu)”
  - https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.1/.github/workflows/build.yml — „"Get-ChildItem "$env:CUDA_PATH\bin\" -Filter "*.dll"" (krok Copy CUDA DLLs)”
  - https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html — „"may be distributed with applications developed by you" (Załącznik A)”
  - https://docs.nvidia.com/cuda/archive/12.4.0/eula/index.html — „"Your application must have material additional functionality, beyond the included portions of the SDK."”
  - https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.1/LICENSE — „"Copyright (c) 2023-2024 The ggml authors"”
- **Uwaga krytyka:** Drobne nadinterpretacje. (1) Wynik pisze, że warunki dla użytkowników mają chronić NVIDIA co najmniej tak jak EULA. Umowa (§1.1.2) wymaga, by warunki były zgodne z umową, w tym z ochroną praw własności intelektualnej NVIDIA, a sformułowania „co najmniej tak jak” w niej nie ma. (2) Wynik nie wspomina, że crates/app-models/src/data.rs:143–145 zapisuje sprawdzenie układu archiwum 06.10.2026 (28 plików). Pełnej listy w repozytorium nie ma, ale wcześniejsze sprawdzenie istnieje. Lista składników CUDA (nvcc, nvrtc, cublas, nvtx, nvprof, cccl) i kopiowanie wszystkich *.dll z CUDA_PATH\bin się potwierdzają.

### `sidecar-ffmpeg`

- **Karta w programie:** LGPL-2.1-or-later OR GPL-2.0-or-later (GNU LGPL 2.1+ albo GNU GPL 2.0+ (zależnie od buildu)), komercyjnie: conditional, wskazanie autora: True.
- **Źródło:** https://ffmpeg.org/download.html (katalog Alfy niczego nie pobiera: Install::Manual, brak plików). FFmpeg.org wydaje tylko kod źródłowy i dla Windows odsyła do kompilacji firm trzecich: https://www.gyan.dev/ffmpeg/builds/ oraz https://github.com/BtbN/FFmpeg-Builds/releases. Dokumenty spike'ów w repo (evals/spikes/*/README.md) zalecają `winget install Gyan.FFmpeg`, czyli kompilację gyan.dev (winget to serwis pośredni).
- **Licencja u źródła:** Kod FFmpeg: domyślnie LGPL-2.1-or-later. Z --enable-gpl: GPL-2.0-or-later. Z --enable-version3: LGPL-3.0 albo GPL-3.0. Z --enable-nonfree: binariów nie wolno rozpowszechniać. Kompilacje dla Windows: gyan.dev to GPLv3 we wszystkich wariantach (essentials, full, full-shared; wydanie 9.0.2 z 2026-09-19). BtbN: warianty lgpl budowane z „--enable-version3” (czyli LGPL-3.0-or-later), warianty gpl z „--enable-gpl --enable-version3” (czyli GPL-3.0-or-later). W bieżącym wydaniu BtbN nie ma wariantów nonfree dla win64.
- **Tekst licencji:** https://ffmpeg.org/legal.html; https://github.com/FFmpeg/FFmpeg/blob/master/LICENSE.md
- **Wydawca:** Projekt FFmpeg (tylko źródła). Kompilacje Windows wydają osoby trzecie: Gyan Doshi (gyan.dev) i BtbN (GitHub BtbN/FFmpeg-Builds).
- **Ograniczenia i warunki:**
  - Lista zgodności z LGPL w legal.html: budowa bez --enable-gpl i --enable-nonfree, łączenie dynamiczne (DLL), udostępnienie dokładnie odpowiadającego kodu źródłowego z linią configure, wzmianka o FFmpeg (strona pobierania, okno „O programie”, EULA), brak zakazu inżynierii wstecznej w EULA, nazwy DLL bez zaciemniania.
  - Kompilacje GPL (gyan.dev, BtbN gpl) wymagają przy rozpowszechnianiu spełnienia warunków GPL-3.0 (tekst licencji, dostęp do źródeł).
  - Kompilacje z --enable-nonfree (np. fdk-aac, OpenSSL) są według LICENSE.md „unredistributable”.
  - Patenty: legal.html ostrzega, że przy produktach komercyjnych możliwe są żądania opłat licencyjnych (wymienia MPEG LA). To kwestia niezależna od licencji praw autorskich.
  - Treść witryny gyan.dev jest na CC BY-NC-ND 4.0. Dotyczy to samej strony, nie kompilacji.
- **Znak wodny:** Nie dotyczy (narzędzie do konwersji, nie TTS).
- **Rozbieżności z kartą:**
  - Wyrażenie SPDX na karcie (LGPL-2.1-or-later OR GPL-2.0-or-later) opisuje możliwe licencje kodu źródłowego, ale nie pasuje do żadnej kompilacji Windows, do której odsyła ffmpeg.org. gyan.dev to GPL-3.0, BtbN lgpl to LGPL-3.0-or-later, BtbN gpl to GPL-3.0-or-later (wszystkie z --enable-version3).
  - Dokumenty repo (evals/spikes/h-sprzet/README.md i evals/spikes/a-petla-glosowa/README.md) zalecają `winget install Gyan.FFmpeg`, czyli kompilację GPLv3. Gdyby to był domyślny build, karta powinna mówić GPL-3.0-or-later.
  - Karta poprawnie stwierdza, że Alfa FFmpeg nie pobiera (Install::Manual). Konkretnej kompilacji nie da się więc ustalić z katalogu: zależy od tego, co skopiuje użytkownik.
- **Dowody:**
  - https://ffmpeg.org/download.html — „"FFmpeg only provides source code."”
  - https://ffmpeg.org/legal.html — „"the GPL applies to all of FFmpeg" (gdy użyto opcjonalnych części GPL)”
  - https://raw.githubusercontent.com/FFmpeg/FFmpeg/master/LICENSE.md — „"will cause the resulting binary to be unredistributable" (--enable-nonfree)”
  - https://www.gyan.dev/ffmpeg/builds/ — „"All builds are 64-bit, static and licensed as GPLv3."”
  - https://raw.githubusercontent.com/BtbN/FFmpeg-Builds/master/variants/defaults-lgpl.sh — „"FF_CONFIGURE=\"--enable-version3 --disable-debug\""”
  - https://raw.githubusercontent.com/BtbN/FFmpeg-Builds/master/variants/defaults-gpl.sh — „"FF_CONFIGURE=\"--enable-gpl --enable-version3 --disable-debug\""”
  - https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest — „ffmpeg-n9.0-latest-win64-lgpl-shared-9.0.zip (nazwa pliku z wydania z 2026-10-21, bez cytatu)”

## Ocena krytyka

Wynik obejmuje wszystkie 24 pozycje z license-cards.json, żadnej nie brakuje. Pola card_license, porównane z license-cards.json, wszędzie zgadzają się z kartami, ale w czterech wpisach TTS tego pola nie ma (supertonic-3, piper-pl_PL-gosia-medium, sidecar-piper, sidecar-pocket-tts). Najbardziej wątpliwe adresy sprawdziłem bezpośrednio (curl i WebFetch, 21.10.2026). Potwierdziły się: - Regulamin bielik.ai v1.1: zakaz przetwarzania danych osobowych, wiek 18+, wiązanie z Apache 2.0. - Qwen2.5-3B na licencji qwen-research (§2(b) dosłownie) i blog Qwen2.5 wyłączający wersje 3B i 72B. - Raport arXiv uzasadniający wybór Qwen2.5 licencją Apache 2.0; „Initialized from” i „Finetuned from” na kartach HF. - Sumy i rozmiary archiwów llama.cpp b11559 i whisper.cpp v1.8.1 oraz PR #18696. - EULA CUDA 12.4: §1.1.2 i Załącznik A. - Licencja DirectML: §1a, §3e, §2a, §6. - Open RAIL-M Supertonic, w tym identyczna suma LICENSE w obu repozytoriach. - Pocket TTS: dostęp warunkowy, model shefowl i daty ADR. - WeSpeaker i VoxCeleb (CC BY 4.0, celów badawczych). - openWakeWord: 5 otwartych zgłoszeń, autor nie odpowiedział w żadnym. - MODEL_CARD Gosi i Lessac, CC0 zbioru, Silero, gyan.dev, BtbN, Smart Turn, Qwen3-ASR. Cytaty mają do 15 słów. Jeden z nich (Privacy.md ONNX Runtime) nie jest dosłowny i pomija dwa zastrzeżenia. Problemy merytoryczne: 1. sidecar-piper: commit espeak-ng 0f65aa30 przypięto dopiero 5.12.2023, po wydaniu. Paczkę z 14.11.2023 zbudowano z master rhasspy/espeak-ng, więc wersja źródeł do udostępnienia zgodnie z GPL jest nieustalona. 2. Telemetria ONNX Runtime: punkt „do potwierdzenia” da się zamknąć. ort 2.0.0-rc.10 domyślnie wywołuje EnableTelemetryEvents, a Alfa woła ort::init_from(...).commit() bez with_telemetry(false) w crates/lib-smart-turn/src/main.rs:44 i crates/lib-supertonic/src/main.rs:62. Piper telemetrię wyłącza. 3. Supertonic: Alfa nie instaluje pliku LICENSE (RAIL-M), mimo że licencja wymaga przekazania kopii. 4. Niespójne kryterium matches_card i niespójne commercial_use dla Bielika 1.5B. 5. Drobne nieścisłości: - Przestrzeń ggerganov nazwana prywatną. - Pominięte osadzenie LICENSE llama.cpp (CMakeLists.txt:198). - Nadinterpretacja §1.1.2 EULA. - Przesadzona rozbieżność z model-cards.json dla Gosi. - Pozorna rozbieżność co do atrybucji VoxCeleb. Strony licencji Lessac (CSTR) nie udało się otworzyć, więc status komercyjny Gosi pozostaje nieustalony. Pliki: E:/PROGRAMY W CZASIE PRAC/Alfa AI Program/alfa/crates/app-models/data/license-cards.json, crates/app-models/src/data.rs, crates/app-models/src/supertonic.rs, crates/app-models/src/voice_runtime.rs, crates/lib-smart-turn/src/main.rs, crates/lib-supertonic/src/main.rs. Nie edytowałem repozytorium i nie pobierałem modeli ani archiwów.

## Co dalej

- Właściciel: decyzje z listy „Najważniejsze” (Regulamin bielik.ai, Qwen Research License dla Bielika 4.5B, DirectML,
  Open RAIL-M Supertonic, FFmpeg, WeSpeaker/openWakeWord) i potwierdzenie kart (`confirmed`).
- Kod: przypięte też rewizje i sumy SHA-256 (LFS oid z API Hugging Face) modeli Whisper (`ggml-large-v3-turbo-q5_0`,
  `ggml-small-q5_1`), `multilingual-e5-small` i `paraphrase-multilingual-MiniLM-L12-v2` (model i `tokenizer.json`; rozmiar
  tokenizera MiniLM poprawiony z 17 na 9 MB), a także głosu Piper Gosia, modelu WeSpeaker i archiwów whisper.cpp i Piper —
  każda pozycja do pobrania ma przypięte sumy (test katalogu); zrobione 2026-10-21.
