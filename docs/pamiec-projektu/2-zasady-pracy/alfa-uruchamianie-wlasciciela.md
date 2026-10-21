---
name: alfa-uruchamianie-wlasciciela
description: "Właściciel uruchamia Alfę tylko skrótem „Alfa” (Alfa-uruchomiona, z Brokerem i oknem zgód); target/shell to build testowy bez Jądra"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 8c1892ed-2987-4f4f-91ab-0abe9bb3c4b8
  modified: 2026-10-21T08:26:09.310Z
---

2026-10-21 właściciel uruchomił ręcznie `alfa\target\shell\debug\alfa-desktop.exe` (ścieżkę podałam
mu wcześniej jako „zbudowaną wersję”) — build testowy nie ma obok programów Jądra, więc Broker działa
w procesie bez okna zgód i każda zmiana poziomu autonomii (L4) była odrzucana. Zgłosił z irytacją
(„znowu brak uprawnień… ma być zrobione raz na zawsze”).

**Why:** `target/shell` celowo jest bez `alfa-broker`/`alfa-broker-ui` (wariant „Broker w procesie”
w testach prawdziwej aplikacji, `deploy-owner.sh` buduje Jądro osobno w `target/safety`). Kopia
właściciela `Alfa-uruchomiona` ma Jądro i działa w trybie przenośnym z oknem zgód.

**How to apply:** właścicielowi wskazywać wyłącznie skrót „Alfa” (pulpit, menu Start →
`Alfa-uruchomiona\alfa-desktop.exe`, tworzy go `scripts/owner-shortcut.ps1`, odświeża
`deploy-owner.sh`); nigdy nie pisać mu ścieżki do `target/shell` jako programu do uruchomienia. Po
budowie zamiast „wersja leży w target/shell” — „gotowa do wgrania przez deploy-owner.sh”. Na nowym
komputerze najpierw sprawdzić, czy skrót istnieje. Zob. [[alfa-samodzielne-testowanie]],
[[maszyna-stacjonarna-2026-10-17]].
