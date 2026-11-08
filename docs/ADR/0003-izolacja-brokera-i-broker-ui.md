# ADR 0003 — Izolacja Brokera i Broker-UI na wyższym poziomie integralności

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (potwierdzenie spike'iem (k) w F0) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §2 (zasada 7), §8.0–8.2, §8.4, §8.6, §13, §16.2 (F0 spike k, F3), §17 |

## Kontekst

Agentki mają „100% dostępu" do komputera (L3 domyślnie, L4 opcjonalnie), a jednocześnie nie mogą same zatwierdzać własnych próśb ani podnosić uprawnień. Model zagrożeń zakłada halucynujący model, wstrzyknięte polecenia (tekst, plik, strona, dźwięk z TV) i XSS → RCE w WebView. Kanał zatwierdzeń musi być odporny na syntetyczne wejście (SendInput) z procesów agentek, a usługi w sesji 0 nie mogą pokazać okna.

## Decyzja

| Składnik | Gdzie | Co robi |
|---|---|---|
| **Usługa Brokera** (`safety-broker`) | proces w tle na **osobnym koncie Windows**, sesja 0 | wydaje tokeny zdolności (TTL, potomek ≤ rodzic), prowadzi zatwierdzenia, **jedyny writer strumienia Audyt** (pliki append-only przez ACL, głowa łańcucha hashy kotwiczona poza zasięgiem agentów), trzyma polityki Jądra, obsługuje kill-switch, uruchamia operacje elewowane przez UAC (opt-in: krótka allowlista z weryfikacją Authenticode+SID) |
| **Broker-UI** (`broker-ui`) | osobny, mały **natywny** proces w sesji użytkownika, uruchomiony na **wyższym poziomie integralności** niż agentki | okno zatwierdzeń i przełącznik poziomów autonomii; **UIPI** blokuje wstrzykiwanie wejścia z procesów agentek; zatwierdzenia tylko z wejścia niewstrzykniętego (fizyczne kliknięcie/klawisz); opcjonalnie Windows Hello (L4, admin, zmiany polityk Jądra) |
| **Watchdog** | osobny proces | kill-switch < 200 ms, Job Objects, safe-mode, rollback |

Zasady towarzyszące:

- Zakaz `gui.control` wobec procesów Alfy, Brokera i helpera `uiAccess` — twarda blokada Jądra także na L4.
- Broker-UI **nie jest WebView z treścią LLM**; treść prośby to sanitizowany tekst.
- Zatwierdzenia **nigdy w toaście** — tylko w Broker-UI.
- Narzędzia wykonujące dla ≤ L3: restricted token / low-integrity / AppContainer.
- Destrukcyjne akcje zlecone głosem wymagają potwierdzenia nie-głosem w Broker-UI na każdym poziomie (§6.10).
- Do F3 Audyt pisze `core-log` z oznaczeniem `pre-broker`; w F3 Broker przejmuje strumień z nowym łańcuchem hashy.
- Zmiany polityk Jądra tylko przez Broker (potwierdzenie w jego oknie).

Spike (k) w F0: uruchomienie Broker-UI na wyższym poziomie integralności z usługi + test odrzucenia SendInput. Kryterium F3: „agentka zmienia Jądro / zatwierdza sama siebie" — ≥ 100 scenariuszy (w tym SendInput do Broker-UI) = 0 sukcesów.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Zatwierdzenia w oknie głównym (WebView2) | ten sam proces i poziom integralności co agentki; XSS → samozatwierdzenie; SendInput z procesu agentki działa |
| Usługa Brokera pokazująca okno sama | usługi w sesji 0 nie mają dostępu do pulpitu użytkownika |
| Otwarta usługa admina dla każdego procesu | eskalacja bez UAC dla dowolnego procesu użytkownika; plan wprost tego zabrania (§8.4) |
| Broker jako wątek w jądrze Alfy | agentki i jądro dzielą proces → polityki i audyt w zasięgu modelu |
| Wyłącznie Windows Hello jako potwierdzenie | Hello jest opcjonalne; domyślnie wystarcza fizyczne wejście, które UIPI chroni |

## Konsekwencje

- Trzy dodatkowe procesy (usługa, Broker-UI, Watchdog); Broker-UI i Watchdog muszą być natywne i minimalne (budżet RAM w manifeście).
- Instalacja usługi na osobnym koncie i helpera `uiAccess` wymaga jednorazowego UAC (bramka ludzka #10).
- Broker nie jest wąskim gardłem logów — pisze tylko Audyt (§13).
- Uczciwie: na L4 bez izolacji audyt jest „best effort" (§8.7).
- Ryzyko: podniesiony poziom integralności Broker-UI a Tauri/WebView2 — poza zakresem, bo Broker-UI nie używa WebView.

## Jak cofnąć

- Gdyby spike (k) wykazał, że uruchomienie procesu o wyższym poziomie integralności z usługi jest niewykonalne: wariant zapasowy to Broker-UI uruchamiany przez launcher z manifestem `uiAccess` (podpis własnym certyfikatem, `Program Files`), nadal poza procesem agentek. Wymaga rewizji tego ADR i `THREAT_MODEL.md`.
- Rezygnacja z osobnego konta usługi obniża gwarancje audytu do „best effort" na wszystkich poziomach — wymaga jawnej zgody właściciela.
