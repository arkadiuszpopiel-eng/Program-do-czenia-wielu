---
name: alfa-testy-poza-ekranem
description: Właściciel 1.11 — testowa Alfa nie może pojawiać się na ekranie ani zabierać fokusu; instancja testowa domyślnie poza ekranem
metadata:
  type: feedback
---

Właściciel 1.11: „Rób tak testy Alfy, by mi nie przeszkadzały — jak ją otwierasz, rób to dla siebie, a nie na całym ekranie”.

Od 1.11 instancja testowa (`ALFA_E2E_INSTANCE=1`) otwiera okna poza monitorami (-16000, -16000), bez paska zadań, fokusu i powiadomień (`cdp::hidden_instance`). Testy wymagające ekranu (maksymalizacja, „Zapisz jako” przez UI Automation, Ctrl+Alt+D/R) tylko z `ALFA_E2E_VISIBLE=1`.

**Why:** okna testowe wyskakiwały na ekran właściciela w czasie jego pracy.
**How to apply:** nigdy nie ustawiać `ALFA_E2E_VISIBLE=1`, gdy właściciel może pracować (tylko w nocy, za jego zgodą); nowe okna i dialogi w powłoce pokazywać przez `windows::present`; testy wymagające ekranu oznaczać `test.skip(HIDDEN, …)`. Zob. [[alfa-samodzielne-testowanie]], [[alfa-praca-nocna]].
