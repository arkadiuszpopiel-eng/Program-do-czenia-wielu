# Protokół nagrań korpusu własnego (bramka ludzka #3; VOICE.md §14)

Łącznie ~30–45 min Twojej mowy, na **desktopie i laptopie**, w 12 typach. Nagrania zostają **lokalnie, poza gitem**
w `evals/corpus/` (`.gitignore` wyklucza `evals/corpus/*` i wszystkie `*.wav`). Do repo trafia tylko lista plików
i hash zamrożonego zestawu testowego (F2).

## Sprzęt i ustawienia

- Mikrofon: desktop — USB/przewodowy (jaki masz); laptop — **wbudowany** (najtrudniejszy przypadek dla AEC) i słuchawki.
- Windows: Ustawienia → Dźwięk → mikrofon → „Ulepszenia dźwięku" **wyłączone** (chyba że typ nagrania mówi inaczej),
  głośność wejścia tak, by szczyty mowy były ok. −12 … −6 dBFS (Audacity pokazuje miernik).
- Audacity (`winget install Audacity.Audacity`): host **WASAPI**, projekt **48 000 Hz**, mono, 24-bit.
- Nie używaj Bluetooth do nagrań (HFP zjeżdża do 16 kHz).

## Format plików

| | Master | Do STT/VAD |
|---|---|---|
| Częstotliwość | 48 kHz | 16 kHz |
| Kanały | mono | mono |
| Głębia | 24-bit PCM WAV | 16-bit PCM WAV |
| Skąd | eksport z Audacity | `ffmpeg -i master.wav -ac 1 -ar 16000 -sample_fmt s16 out16k.wav` |

Konwersja wsadowa całego katalogu:

```powershell
Get-ChildItem "$HOME\Alfa\evals\corpus\raw" -Recurse -Filter *.wav | ForEach-Object {
    $out = $_.FullName -replace '\\raw\\', '\16k\'
    New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
    ffmpeg -loglevel error -y -i $_.FullName -ac 1 -ar 16000 -sample_fmt s16 $out
}
```

## Nazewnictwo i układ katalogów

```
evals/corpus/
  raw/<maszyna>/<RRRR-MM-DD>/t<NN>-<typ>-<warunki>-<mic>-<nr>.wav     master 48 kHz
  16k/<maszyna>/<RRRR-MM-DD>/...                                       to samo po konwersji
  transcripts/<ta sama nazwa>.txt                                      referencyjna transkrypcja (UTF-8, 1 wypowiedź = 1 linia)
  tts-candidates/<kandydat>/<kandydat>-NN.wav                          próbki TTS do ślepej oceny
  tts-blind/                                                           przemieszane próbki
  spike-a/                                                             nagrania tur ze spike'u (a)
  INDEX.md                                                             lista plików, warunki, czas trwania (można skopiować do wyników)
```

- `<maszyna>`: `desktop` | `laptop`
- `<NN>`: numer typu 01–12 (tabela niżej); `<typ>`: skrót z tabeli
- `<warunki>`: `cisza` | `szum` | `glosniki` | `sluchawki`
- `<mic>`: `usb` | `wbud` | `sluch`
- `<nr>`: kolejny plik w tym typie (01, 02…)

Przykład: `raw/laptop/2026-10-05/t03-swobodna-glosniki-wbud-01.wav`.

## 12 typów nagrań (VOICE.md §14) — jak nagrać

| # | Skrót | Co mówić | Warunki | Czas | Wskazówki |
|---|---|---|---|---|---|
| 1 | `swobodna` | Opowiadaj o dniu, zadawaj pytania asystentce („jaka jutro pogoda", „przypomnij mi…"), czytaj fragment artykułu | cisza, mikrofon USB (desktop) | 5 min | naturalne tempo, pauzy, „yyy" też zostawiaj — to prawdziwa mowa |
| 2 | `swobodna` | jw. | **szum**: wentylator, otwarte okno na ulicę, cicha muzyka w tle (nie z tego komputera) | 4 min | zapisz w INDEX, co szumiało |
| 3 | `swobodna` | jw., **na laptopie**, przez **wbudowany mikrofon**, przy odtwarzaniu z **głośników laptopa** dowolnego TTS/podcastu na 50 % głośności | głośniki + wbud. mic | 4 min | to jest test AEC; mów w przerwach i „na" głośniki |
| 4 | `swobodna` | jw., w słuchawkach (mikrofon słuchawek lub wbudowany) | słuchawki | 3 min | |
| 5 | `plen` | Terminy IT i nazwy produktów po angielsku w polskich zdaniach: „zrób git rebase na main", „otwórz Visual Studio Code", „pull request", „deploy na staging", czytaj 10 linii kodu na głos | cisza | 4 min | mieszaj wymowę „polską" i „angielską" |
| 6 | `nazwy` | Nazwy plików i folderów z Twojego dysku, imiona i nazwiska znajomych (mogą być zmyślone), aplikacje, miasta, ulice | cisza **i** szum (po 1,5 min) | 3 min | powtórz każdą 2 razy |
| 7 | `komendy` | Komendy `voice-cmd`: **„stop"** i **„anuluj"** ≥ 200 razy łącznie (różna głośność, tempo, z odległości 0,5 m i 2 m), plus po 20×: „pauza", „głośniej", „ciszej", „wycisz mikrofon", „przełącz na Deltę", „powtórz", „wznów" | różne | 4 min | nudne, ale to daje recall ≥ 99 %; rób serie po 20 |
| 8 | `przerwania` | Podczas odtwarzania TTS (dowolny długi tekst z głośników / słuchawek) wchodź w słowo: korekta („nie, nie ten"), uzupełnienie („i jeszcze…"), pytanie („a ile to kosztuje?"), zmiana tematu, „stop", „kontynuuj" — **≥ 50 na klasę** | głośniki i słuchawki | 6 min | zapisz w INDEX klasę każdej próby (albo mów numer klasy przed próbą: „jeden… nie, nie ten") |
| 9 | `backchannel` | Podczas odtwarzania TTS: „mhm", „tak", „aha", „no", „nie no, dobrze", „okej" — krótkie, bez zamiaru przerwania | głośniki | 2 min | |
| 10 | `wake` | „Hej Alfa", „Hej Beta", „Hej Gama", „Hej Delta" — ≥ 200 pozytywów łącznie, 0,5 m / 2 m / 4 m, szeptem, normalnie, głośno | cisza + szum | 4 min | odległości zapisz w INDEX |
| 11 | `enroll` | Czytanie 2 min tekstu + 1 min swobodnie; **normalnie i szeptem** (po połowie) | cisza | 3 min | do weryfikacji mówcy; nagrać na obu maszynach |
| 12 | `dyktowanie` | Dyktuj e-mail z interpunkcją mówioną: „przecinek", „kropka", „nowa linia", „w cudzysłowie", „wielka litera" | cisza | 3 min | |

Osobno (nie Twoja mowa, tylko do FAR w F5): ≥ 24 h tła PL (TV, podcasty) — nagrywać dopiero w F5, nie teraz.

## Transkrypcje referencyjne (do WER, typy 1, 2, 5, 6)

Dla każdego pliku typu 1/2/5/6 plik `transcripts/<nazwa>.txt`: dokładnie to, co powiedziano, **liczby słownie tak, jak
wymówione**, bez interpunkcji nie ma znaczenia (normalizacja ją usuwa), wtrącenia EN w oryginalnej pisowni
(`pull request`, nie „pul rekłest"). Najprościej: przepisz sam/sama słuchając; jeśli użyjesz STT jako brudnopisu,
**popraw każde słowo** — inaczej WER będzie kłamał na korzyść tego STT.

## Podział dev/test (F2, ale zapisz od razu)

Pliki z nieparzystym `<nr>` → `dev`, parzystym → `test`. Zestaw `test` zostaje zamrożony hashem przed implementacją F2
(`evals/acceptance/HASHES` dostaje hash listy nazw + SHA-256 plików, same pliki poza gitem).

## Co skopiować z powrotem

- `evals/corpus/INDEX.md` (lista plików, czasy trwania, warunki, mikrofon, maszyna) — **ten plik można** wkleić do sesji;
- nic więcej z `evals/corpus/` nie wysyłaj do repo ani do sesji AI (nagrania są danymi biometrycznymi — VOICE.md §13).
