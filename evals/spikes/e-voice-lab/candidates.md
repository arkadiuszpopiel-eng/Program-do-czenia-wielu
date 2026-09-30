# Tabela kandydatów STT/TTS (spike (e)) — do wypełnienia pomiarem

Skopiuj ten plik do `results/<maszyna>-<data>.md` i wypełniaj. Wiersze pochodzą z `docs/VOICE.md` §15.2; „?" zastępujesz
wartością lub „brak PL" / „brak klucza" / „nie mierzono (powód)". Wersje i hashe modeli — obowiązkowo.

Legenda kolumn: **PL** — obsługa polskiego (tak/nie/z akcentem); **Licencja** — kodu i modelu (np. MIT / CC-BY-4.0 / komercyjna);
**Streaming** — audio lub tekst fragmentami; **Zasoby** — Private WS [MB] / VRAM [MB] / backend (CPU, Vulkan, CUDA);
**TTFB** — czas do pierwszego fragmentu [ms] (TTS) lub opóźnienie finalizacji [ms] (STT); **Ocena** — ślepa ocena 1–5 (TTS).

## STT

| Kandydat | PL | Licencja | Streaming | Zasoby desktop | Zasoby laptop | Opóźnienie finalizacji desktop/laptop [ms] | WER PL (korpus, cisza / szum / łącznie) | Wersja / model / hash | Uwagi |
|---|---|---|---|---|---|---|---|---|---|
| whisper.cpp large-v3 (pełny) | tak | MIT / OpenAI | partial przez okna | ? | ? (czy mieści się w 6 GB?) | ? | ? | | domyślny D-AMD16 |
| whisper.cpp large-v3-turbo Q5_0 | tak | MIT / OpenAI | jw. | ? | ? | ? | ? | | domyślny A/B/D-CUDA; halucynacje na ciszy? |
| whisper.cpp small Q5 (CPU) | tak | MIT / OpenAI | jw. | ? | ? | ? | ? | | fallback |
| Parakeet v3 (ONNX, sherpa-onnx) | tak | ? (sprawdź kartę modelu) | ? | ? (CPU) | ? | ? | ? | | |
| ElevenLabs Scribe v2 RT | tak | chmura | tak | — | — | ? | ? | | brak klucza? |
| Soniox | tak | chmura | tak | — | — | ? | ? | | |
| gpt-4o-transcribe | tak | chmura | ? | — | — | ? | ? | | |
| Qwen3-ASR | tak | chmura | ? | — | — | ? | ? | | tag jurysdykcji CN |

## TTS

| Kandydat | PL | Licencja | Streaming | Zasoby desktop | Zasoby laptop | TTFB desktop/laptop [ms] | RTF desktop/laptop | Ocena 1–5 (średnia z 20) | Klon / voice design | Wersja / model / hash | Uwagi |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Pocket TTS + model PL społeczności | tak | CC-BY-4.0 (?) | tak | ? (CPU) | ? | ? | ? | ? | klon z referencji | | domyślny lokalny |
| Piper pl_PL (głos: ?) | tak | ? | nie | ? (CPU) | ? | ? | ? | ? | nie | | zapas |
| Chatterbox Multilingual | tak (akcent?) | ? | ? | nie mierzono (brak CUDA) | ? (CUDA, VRAM?) | ? | ? | ? | tak | | tylko profil D-CUDA |
| XTTS-v2 | tak | ? | ? | nie mierzono | ? | ? | ? | ? | tak | | tylko profil D-CUDA |
| F5-TTS | ? | ? | ? | nie mierzono | ? | ? | ? | ? | tak | | |
| VibeVoice-Realtime | ? | ? | ? | ? | ? | ? | ? | ? | ? | | do sprawdzenia |
| ElevenLabs | tak | chmura | tak, znaczniki słów | — | — | ? | — | ? | Voice Design + klon | | brak klucza? |
| Cartesia | tak | chmura | tak | — | — | ? | — | ? | ? | | |
| Google Chirp 3 HD | tak | chmura | ? | — | — | ? | — | ? | ? | | |
| Azure pl-PL | tak | chmura | tak | — | — | ? | — | ? | ? | | |
| Gemini TTS | tak | chmura | ? | — | — | ? | — | ? | voice design (ToS?) | | |
| OpenAI TTS | tak | chmura | tak | — | — | ? | — | ? | nie | | |
| MiniMax Speech 2.8 | tak | chmura | ? | — | — | ? | — | ? | klon | | tag jurysdykcji |

## Pozostałe (VAD / koniec tury / KWS / mówca) — tylko „działa / nie działa" i CPU

| Składnik | Sprawdzone? | CPU [ms na ramkę] | Uwagi |
|---|---|---|---|
| Silero VAD | | | |
| Smart Turn v3.2 (PL?) | | | |
| openWakeWord (EN → własny trening) | | | tylko notatka o wykonalności |
| WeSpeaker / ECAPA (sherpa-onnx) | | | |

## Zdania testowe PL (20) — te same dla RTF, ślepej oceny i pętli głosowej

Zawierają liczby, daty, godziny, nazwy własne, adresy, terminy IT i wtrącenia EN, cudzysłowy i pytania.

1. Dzień dobry, jestem Alfa. W czym mogę dzisiaj pomóc?
2. Spotkanie z panią Katarzyną Wiśniewską przesunięto na czwartek, 12 marca 2026 roku, na godzinę 14:30.
3. Plik raport_kwartalny_final_v3.xlsx ma 2,7 megabajta i został zmieniony wczoraj o 23:58.
4. Zamów dwadzieścia trzy sztuki po 1499 złotych i 99 groszy; razem wychodzi 34 499 złotych i 77 groszy.
5. Uruchom pull request numer 418 w repozytorium alfa-desktop i sprawdź, czy CI jest zielone.
6. Temperatura w Zakopanem spadnie w nocy do minus 17 stopni, a w Szczecinie będzie około 3 stopni.
7. Nie, nie ten plik. Miałam na myśli folder „Zdjęcia z Chorwacji 2024”, nie „Zdjęcia 2025”.
8. Przełącz na Deltę i poproś ją o podsumowanie ostatnich pięciu wiadomości od Marka Grzegorzewskiego.
9. Serwer odpowiedział błędem 503 Service Unavailable; spróbuję ponownie za trzydzieści sekund.
10. W Bydgoszczy, Rzeszowie i Świnoujściu jutro zapowiadają przelotne opady i wiatr do 60 kilometrów na godzinę.
11. Zainstaluj Node dwadzieścia dwa, potem wpisz pnpm install i uruchom build.
12. Przypomnij mi o dentyście w poniedziałek o 8:15 rano i o wizycie u Grzegorza we wtorek po południu.
13. Ostatnia aktualizacja sterownika Adrenalin pochodzi z 3 września; nowsza wersja nie została jeszcze wydana.
14. Przepraszam, nie dosłyszałam. Czy chodziło o „wyślij”, czy o „wyszlij”?
15. Mój adres to ulica Świętokrzyska 31/33, mieszkanie 7, 00-049 Warszawa.
16. Cztery agentki, Alfa, Beta, Gama i Delta, mogą pracować równolegle, ale mówi tylko jedna naraz.
17. Backup zajął 4 minuty i 12 sekund; skopiowano 18 432 pliki, pominięto 3 z powodu braku uprawnień.
18. Otwórz w Excelu arkusz „Budżet 2026” i zaznacz komórki od B2 do F14.
19. Rzeczywisty czas przetwarzania wyniósł zero przecinek dwadzieścia jeden, czyli pięć razy szybciej niż w czasie rzeczywistym.
20. Dobranoc. Wyłączam mikrofon; jeśli będziesz czegoś potrzebować, naciśnij skrót lub powiedz „Hej Alfa”.

Te same zdania są wpisane w `h-sprzet/bench_pocket_tts.py`. Dla innych silników zapisz je do `zdania.txt` (jedno na linię,
UTF-8) i podawaj po kolei.

## Protokół ślepej oceny TTS (F0-09)

Wymagania: ≥ 3 kandydatów × 20 zdań, ten sam tekst, ta sama głośność (znormalizuj do −16 LUFS: `ffmpeg -af loudnorm=I=-16`),
słuchawki, jedna sesja ≤ 45 min (potem ucho się męczy), bez patrzenia na nazwy plików.

1. Pliki kandydatów: `evals\corpus\tts-candidates\<kandydat>\<kandydat>-01.wav … -20.wav` (poza gitem).
2. Przemieszanie i ukrycie nazw (pwsh, z katalogu głównego repo):

```powershell
$src = "$HOME\Alfa\evals\corpus\tts-candidates"
$dst = "$HOME\Alfa\evals\corpus\tts-blind"
New-Item -ItemType Directory -Force $dst | Out-Null
$files = Get-ChildItem $src -Recurse -Filter *.wav | Sort-Object { Get-Random }
$map = @()
$i = 0
foreach ($f in $files) {
    $i++
    $new = ('probka-{0:D3}.wav' -f $i)
    Copy-Item $f.FullName (Join-Path $dst $new)
    $map += [pscustomobject]@{ probka = $new; kandydat = $f.Directory.Name; zdanie = ($f.BaseName -replace '.*-(\d+)$', '$1') }
}
$map | Export-Csv "$dst\..\mapa-$(Get-Date -Format yyyy-MM-dd).csv" -NoTypeInformation -Encoding UTF8
"probka,ocena,uwaga" | Set-Content "$dst\..\ocena-$(Get-Date -Format yyyy-MM-dd).csv" -Encoding UTF8
"Wygenerowano $i probek w $dst. NIE otwieraj pliku mapa-*.csv przed ocena."
```

3. Skala (dla każdej próbki jedna liczba w kolumnie `ocena` pliku `ocena-<data>.csv`):

| Ocena | Znaczenie |
|---|---|
| 5 | brzmi jak lektorka-człowiek; poprawna wymowa, akcent i intonacja; liczby/daty/nazwy poprawnie |
| 4 | drobne potknięcie (jedno słowo, lekko sztuczna intonacja), nic nie przeszkadza |
| 3 | słychać, że syntezator; 1–2 błędy wymowy albo zła intonacja pytania; zrozumiałe |
| 2 | kilka błędów, obcy akcent, przekręcone liczby/nazwy; męczące |
| 1 | niezrozumiałe fragmenty, artefakty, urwania |

Dodatkowo w kolumnie `uwaga` wpisuj krótko: „liczba źle", „akcent EN", „za szybko", „PL/EN ok".

4. Po ocenie (nie wcześniej) połącz z mapą i policz średnie:

```powershell
$d = Split-Path $dst
$o = Import-Csv "$d\ocena-<data>.csv" ; $m = Import-Csv "$d\mapa-<data>.csv"
$j = foreach ($r in $o) { $mm = $m | Where-Object probka -eq $r.probka ; [pscustomobject]@{ kandydat = $mm.kandydat; ocena = [double]$r.ocena } }
$j | Group-Object kandydat | ForEach-Object { [pscustomobject]@{ kandydat = $_.Name; srednia = [math]::Round(($_.Group.ocena | Measure-Object -Average).Average, 2); n = $_.Count } } | Format-Table
```

5. Wynik (średnie per kandydat, n, data, słuchawki) wpisz do kolumny „Ocena 1–5" w tabeli TTS i skopiuj oba `.csv`
   do `evals/spikes/e-voice-lab/results/`. Go: najlepszy kandydat ≥ 4,0. No-go: zostają głosy v0 (Pocket/Piper), nie blokuje F1.
