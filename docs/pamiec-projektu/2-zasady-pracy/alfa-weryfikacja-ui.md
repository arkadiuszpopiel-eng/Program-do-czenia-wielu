---
name: alfa-weryfikacja-ui
description: Alfa — przed oddaniem zmian UI sprawdzać każdy stan interaktywny w każdym stylu i motywie; żadnych kontrolek systemowych
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-09T10:27:09.790Z
---

Właściciel (Arkadiusz), 2026-10-09, po wersji 0.0.3: w stylu „Szkło” rozwinięta lista wyboru miała niewidoczne
napisy (jasny tekst na białym tle), a same listy rozwijane były systemowe (natywny `<select>`), niezgodne ze
standardem interfejsu. Jego słowa: takie błędy „nie mogą się zdarzać w zaawansowanym programie i produkcji”,
mam „wszystko sprawdzać wiele razy i nie przepuszczać takich pomyłek UI i innych ważnych funkcji”.

**Why:** oddałem style po obejrzeniu zrzutów ze stanem spoczynkowym; rozwiniętej listy nie otworzyłem w żadnym stylu.
Axe i zrzuty Playwrighta nie widzą listy natywnego `<select>` (rysuje ją system poza stroną), więc zielone testy
niczego tu nie dowodziły.

**How to apply:**

- Zmiana wyglądu = przejście **każdej kontrolki w stanie otwartym/aktywnym** (lista rozwinięta, menu, podpowiedź,
  okno dialogowe, fokus, hover, wyłączona, błąd) w **każdym stylu × jasny/ciemny**, zanim powiem „gotowe”.
  Zrzut stanu spoczynkowego to za mało.
- Sprawdzać w prawdziwej aplikacji (WebView2), nie tylko na atrapie w przeglądarce — i dopiero wtedy raportować.
- Żadnych kontrolek rysowanych przez system (natywne listy `<select>`, systemowe menu, paski przewijania bez
  stylu) — wszystko ma być w stylu Alfy (`packages/ui-kit`), spójne z resztą.
- Każdy taki błąd dostaje test, który by go złapał (np. e2e otwierający listę i sprawdzający kontrast opcji).
- Dotyczy też „innych ważnych funkcji”: nie zgłaszać ukończenia po jednym przejściu — sprawdzić drugi raz inną
  drogą (inny rozmiar okna, inny styl, prawdziwa aplikacja). Powiązane: [[alfa-zasady-pracy]].
