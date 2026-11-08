# 51 — Raport: wieczór 3.11 (etap Z — tryby zgód jak w Claude Code i Codex)

**W skrócie.** Polecenie właściciela 3.11 (ok. 18:45): „pracuj automatycznie do 20:00, potem kopia na F:”. Zadanie:
zgody i uprawnienia na tej samej zasadzie co Claude Code i Codex (plan [50](50-tryby-zgod-jak-claude-codex.md)).
Zrobione:

- tryb **„Akceptuj edycje”** w Sędzim Cieni: zmiany w folderze pracy rozmowy przechodzą bez pytania, a poza nim
  wymagają zgody, jak „Poproś o zatwierdzenie” w Codex;
- **ścieżki chronione**: `.git`, `.ssh`, `.vscode`, pliki startowe powłoki i inne. Ich zmiana czeka na zgodę we
  wszystkich trybach poza Pełnym dostępem;
- menu z **pięcioma trybami** pod nazwami jak w obu programach.

Wszystko leży na gałęzi `do-przegladu-tryby-zgod`, **nie jest scalone**, bo kod Sędziego wymaga przeglądu człowieka.

## 1. Co się zmieniło

| Zmiana | Commit | Dla właściciela |
| --- | --- | --- |
| Folder pracy rozmowy jako fakt dla Sędziego; reguły „poza folderem pracy” (do L2) i „ścieżka chroniona” (do L3, bez stałej zgody) | deaaf0a1 | W „Akceptuj edycje” agentka sama zmienia pliki w folderze rozmowy, a o pliki poza nim pyta |
| Poprawki z przeglądu: aliasy 8.3 (`GIT~1`), folder pracy w katalogu `.claude`, ścieżki chronione w poleceniach powłoki (`Set-Content .git\hooks\…`) | 72214cc2 | Ochrony nie da się obejść skróconą nazwą ani poleceniem w konsoli |
| Menu „Tryb pracy”: 5 trybów, klawisze 1–5 | f5f8a5c5 | Nowa pozycja „Akceptuj edycje” |
| Nazwy: **Ręcznie** (L1), **Akceptuj edycje** (L2), **Pełny dostęp** (L4) | 38047a86 | Nazwy jak w Claude Code i Codex — w menu, oknie Sędziego i Ustawieniach |
| Powiadomienia nad paskiem pisania | c83da6a0 | Dwa powiadomienia naraz nie zasłaniają już żetonu trybu ani przycisku wysyłania |
| Test własności: folder pracy nie daje więcej uprawnień, niż daje profil | 10abed5a | Gwarancja, że nowy tryb niczego nie poluzował |

## 2. Testy

Wszystkie poniższe przeszły:

- tabela klasyfikatora: 89 przypadków × 5 poziomów;
- własności klasyfikatora i Sędziego (także z nowymi faktami);
- `safety-broker-contract`: 4 testy folderu pracy;
- `tools-shell` (regresja ścieżek chronionych w poleceniach);
- tools-fs, tools-common, safety-broker-impl/fake, broker-ui, app-chat;
- vitest: 543/543;
- svelte-check: 0 błędów;
- Playwright (Edge): menu trybu, panele, połączenia, błędy — 27/27 w każdym stylu i motywie; pełny zestaw
  przeglądarkowy po wszystkich zmianach — **771/771** (11,4 min).

Pełnej bramki `verify wersja` nie uruchamiałam: zmiany czekają na przegląd, a wersja i budowa wypadają co 10 punktów.

## 3. Przegląd w świeżym kontekście

Wynik przeglądu:

- nic krytycznego;
- brak regresji: nowe fakty tylko dodają pytania;
- folder pracy nie poszerza zakresu ponad profil.

Luki nowej ochrony i ich stan opisuje plan 50 §8. Poprawione: aliasy 8.3, folder pracy w katalogu chronionym, ścieżki
chronione wskazane w poleceniu. Otwarte, do decyzji:

1. usuwanie katalogu nadrzędnego wobec `.git`;
2. dowiązania (junction) w folderze pracy prowadzące poza niego;
3. zapis pośredni, np. `git commit` pisze do `.git` bez podania ścieżki — Codex rozwiązuje to piaskownicą zapisu.

## 4. Dalej (etap Z)

1. **Max dostęp:** czerwony przełącznik z wyborem lokalne / chmura / wszystkie. Projekt jest w planie 50 §8.1 —
   Sędzia musi znać pochodzenie modelu przebiegu.
2. **Z.3:** okno zgody z 4 opcjami i zakresem „nie pytaj więcej o …”.
3. **Z.4:** model-sędzia w trybie „Automatycznie” z bezpiecznikiem 3/20.
4. **Z.5:** tryb Plan, który zastąpi „Tylko podgląd”.
5. **Z.6–Z.7:** profil Dziecko z kłódkami, PIN rodzica i dziennik rodzica.

## 5. Do decyzji właściciela

- Przegląd i scalenie gałęzi `do-przegladu-tryby-zgod`: zmiany w `safety-broker-contract`, `risk-classifier-contract`
  i `broker-ui-contract`.
- Czy zapis pośredni do `.git` (np. `git commit`) ma pytać w „Akceptuj edycje” i „Automatycznie”, jak w Codex.

Kopia czysta na F: — patrz `memory/status.md` (wykonana po 20:00).
