# Rzeczywisty odczyt TEMP i kolejna tura — dowody

Wybór fs_read w pierwszej turze był skryptowany; wykonawcaCore odczytał rzeczywisty,
świeży losowy tekst z własnegoTEMP. ToolResult zawierał ten tekst, Replay mafs_read,
plik nie zmienił się. Core odbudowany nad tą samą bazą/sejfemMemory; katalog roboczy
wyłączony. Następne żądanie ma1RecordedDone/0Unverified i fs_read/native_replay,
bez zawartości pliku; bieżąca tura nie uruchamia narzędzi.

- Kontrola danych1/0/1ignored; regresje context/live/actual_read **5/0/2ignored**.
- Clippy app-core/all-targets0bezostrzeżeń. fmt/diff/YAML0.
- Jedna rzeczywista próbaDelta/Standard naQwen8B/CPU4: **input_ok=true**,0tekstu,
  FirstToken60s,elapsed99337ms, kod101. **SemantykaNOT_EVALUATED**. Bez ponowienia/progu.
- Startup/warm-cache nie jest baseline: przed próbą czytano wagi w celu weryfikacjiSHA.
- Model/serverSHA_MATCH; parametry96/temp0/8192, produkcyjne zegary niezmienione.
- Supervisor180s nie zadziałał: test sam skończył101. Serwer203585 po stopie byłzombie,
  proces testu203423 i supervisor203416 nie istniały; brak aktywnego obciążenia modelu.

Oryginalny native-report.json zachowany bez edycji. Jego ogólne pole semantic_acceptance
brzmi manual review required; przy braku tekstu obowiązujeNOT_EVALUATED. Przed publikacją
kod raportu poprawiono, by to rozróżniał, oraz doprecyzowano sprawdzenieRole::User.
Nie ponawiano modelu po tych dwóch zmianach, które nie zmieniają wejścia/inferencji.
Końcowy kod ma hashe w actual-read-final-source.json; regresje iClippy dotyczą końcowego kodu.

Windowsowy odpowiednik przygotowany po starej macierzy30, z osobnymactual-read-screen.json
w istniejącym artefakcie. Nie uruchamia się z harmonogramu; bez continue-on-error i zmian120min.
Brak wyniku tego testu naWindows/GPU. Niezależny przegląd i noweCI pozostają wymagane.
