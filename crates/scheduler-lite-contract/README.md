# scheduler-lite-contract

Kontrakt `scheduler-lite` (docs/modules/scheduler-lite/SPEC.md, PLAN §9.3–9.4, §6.5): deterministyczny
menedżer **zasobów wyłącznych** — `Speaker` (głośnik/mówienie), `Mic`, `ScreenInput`, `File(ścieżka)`.

- `acquire(LeaseRequest { resource, holder, priority, max_wait, on_timeout }) -> Lease` — dzierżawa
  **RAII** (zwolnienie przy `drop`); kolejka priorytetowa (priorytet malejąco, potem FIFO).
- **Voice-first**: czekające żądanie o wyższym priorytecie (np. mowa użytkownika `UserSpeech`)
  natychmiast ustawia posiadaczce sygnał `PreemptRequested` — tylko gdy zasób ma
  `preemptible_at_atomic`; posiadaczka **nie jest zabijana**, sama zwalnia w punkcie atomowym.
  `preempt(resource, by, reason)` robi to jawnie; `KillSwitch` odbiera dzierżawę.
- **Timeout**: `max_wait` zawsze skończone (≤ 600 s; 0 = próba), po czasie `SchedError::Timeout`
  z `on_timeout: ask_user|fail` (domyślnie: ekran → `ask_user`, reszta → `fail`).
- **Kolejka mówienia i przekazanie bez luki**: `Lease::handoff(to)` / `handoff(resource, from, to)` —
  czekająca adresatka dostaje zasób w tej samej chwili (przed innymi); jeśli jeszcze nie prosi,
  zasób jest dla niej zarezerwowany (`handoff_reserve_ms`, domyślnie 2 s).
- **Zakleszczenia**: graf oczekiwania (posiadaczki); cykl → `SchedError::Deadlock` dla
  **najmłodszego** żądania w cyklu (sprawdzane po każdym zakolejkowaniu i przyznaniu).
- Usługi systemowe (`Holder::System`) nie mogą trzymać `Speaker`; kill-switch (`kill_all`) odbiera
  wszystko i czyści kolejki.
- Zdarzenia: `scheduler.lease.granted/released/preempted/timeout` + `queued/handoff/deadlock/
  revoked/cancelled` (z `agent` = persona posiadaczki).

**Rdzeń decyzyjny jest częścią kontraktu** (`LockTable` — synchroniczny, bez zegara; `Core<H: Host>` —
sterownik async z oneshot/watch): SPEC wymaga determinizmu („ta sama sekwencja żądań = ta sama
kolejność przyznań”), a `-impl` (zegar tokio + magistrala) i `-fake` (wirtualny zegar + nagranie)
różnią się tylko `Host`em. Wyniki dostarczane są po zwolnieniu blokady (drop niedostarczonej dzierżawy
nie zakleszcza sterownika).

Testy: własności — **1000 losowych scenariuszy** (5 posiadaczek, 3 zasoby, żądania/zwolnienia/czas/
wywłaszczenia/przekazania/punkty atomowe/kill-switch): wyłączność w każdej chwili, graf oczekiwania
bez cykli, kolejka uporządkowana, nikt nie czeka po terminie, każde żądanie rozstrzygnięte dokładnie
raz, na końcu wszystko zwolnione; test pokrycia (w 300 scenariuszach występują wszystkie rodzaje
decyzji, w tym ~65 rozwiązanych zakleszczeń); determinizm. Kontraktowe (7 przypadków) pod feature
`contract-tests` z uprzężą `Harness` sterującą czasem.
