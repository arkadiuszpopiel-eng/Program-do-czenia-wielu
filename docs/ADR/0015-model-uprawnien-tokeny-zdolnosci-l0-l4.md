# ADR 0015 — Model uprawnień: tokeny zdolności i poziomy autonomii L0–L4

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.1 (pkt 11), §1.4, §2 (zasada 6), §6.10, §8.1, §8.3, §8.7, §9.2, §9.4, §12.1, §14.6, §16.2 (F3) |

## Kontekst

Właściciel chce autonomii „bardzo wysokiej lub maksymalnej", zmiennej w ustawieniach, a jednocześnie „100% dostępu" nie może oznaczać, że błąd modelu lub wstrzyknięte polecenie skasuje dane, wyśle sekrety albo podniesie własne uprawnienia. Role agentek są zmienne (obsada), więc uprawnienia nie mogą być przypisane do persony.

## Decyzja

**Tokeny zdolności** wydawane przez Broker (ADR 3): `fs.read/write(zakres)`, `shell.exec`, `gui.control(aplikacja)`, `net.egress(host)`, `secrets.read`, `system.admin`; z TTL; **potomek ≤ rodzic**; reguły Marszałka i „zawsze zezwalaj w tym zakresie" mogą tylko zawężać. **Uprawnienia idą za rolą, nie za personą**, pod sufitem sesji: przy zmianie obsady Broker wydaje nowe tokeny (Krytyczka tylko odczyt, Badaczka w izolacji na niezaufanych źródłach, Wykonawczyni z narzędziami systemowymi).

| Poziom | Nazwa | W praktyce |
|---|---|---|
| L0 | Podgląd | tylko czyta i podpowiada |
| L1 | Pytaj o wszystko | każda zmiana wymaga „tak" |
| L2 | Pytaj o ryzykowne | drobiazgi sama; pyta przy usuwaniu, wysyłaniu danych na zewnątrz, instalacji |
| **L3** | **Bardzo wysoka (domyślny)** | działa sama w profilu użytkownika i wskazanych aplikacjach; pyta przy nieodwracalnych poza zakresem albo przy działaniu na podstawie niezaufanej treści |
| **L4** | **Maks** | nie pyta o nic poza twardymi blokadami Jądra i potwierdzeniem destrukcji zleconej głosem; jeden przełącznik w Ustawieniach (globalnie / per sesja / per agentka; opcjonalnie na czas) |

Reguły niezależne od poziomu:

- Poziom jest per sesja i per agentka; **agentka nie może podnieść własnego poziomu** — robi to Broker po fizycznym potwierdzeniu w Broker-UI (opcjonalnie Windows Hello).
- **Nawet na L4 zostają:** kill-switch, dziennik cofania, audyt, twarde blokady (wyłączenie audytu, formatowanie dysku systemowego, `gui.control` wobec Alfy/Brokera/helpera, deny-listy poświadczeń z §1.3).
- Destrukcyjne akcje zlecone głosem: potwierdzenie nie-głosem na każdym poziomie (wyjątek tylko ręcznie w Broker-UI).
- Sesja `tainted` po pierwszym niezaufanym wejściu: wysokie ryzyko i `net.egress` wymagają potwierdzenia; „lethal trifecta" (dane prywatne + niezaufana treść + kanał wyjścia) nie współistnieje bez potwierdzenia.
- Klasyfikator ryzyka: odwracalność, zakres, wpływ zewnętrzny, destrukcyjność, pewność STT. Flaga `reversible: yes|scoped|no` w manifeście narzędzia.
- Narzędzia wykonujące dla ≤ L3: restricted token / low-integrity / AppContainer.
- Zmęczenie zatwierdzeniami: „plan do zatwierdzenia" zamiast 40 pytań, szablony uprawnień, metryka pytań/godz.; „zawsze zezwalaj" nie eskaluje do L4.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Uprawnienia per persona | rola się zmienia, persona nie; Krytyczka-Gama i Wykonawczyni-Gama potrzebują innych praw |
| Dwa poziomy (pytaj / nie pytaj) | brak miejsca na „pytaj o ryzykowne" i na niezaufaną treść jako czynnik |
| L4 bez twardych blokad | błąd STT lub injection może skasować dane; właściciel zaakceptował blokady jako ochronę, nie ograniczenie |
| Uprawnienia w konfiguracji agentki, egzekwowane w procesie agentki | agent mógłby je zmienić; egzekucja musi być poza procesem (Broker) |
| Windows Hello obowiązkowe | bramka ludzka niepotrzebna na co dzień; fizyczne wejście w Broker-UI wystarcza, Hello opcjonalne |

## Konsekwencje

- W F2 rola = prompt + polityka modelu; tokeny zdolności per rola dochodzą z Brokerem w F3.
- Kryteria F3: cofalność ≥ 200 losowych operacji `fs.*` 100%; „agentka zmienia Jądro / zatwierdza sama siebie" ≥ 100 scenariuszy = 0 sukcesów; red-team injection ≥ 100 przypadków: 0 eskalacji i 0 egressu bez potwierdzenia.
- Ulepszacz (R0–R2) nie może zmieniać tagów prywatności, budżetów, uprawnień, egress-allowlisty ani progów bramki.
- Uczciwie: na L4 bez izolacji audyt jest „best effort" (§8.7); Credential Manager/DPAPI chroni przed kradzieżą offline, nie przed procesem tego samego użytkownika.
- UI: karta „czeka na zatwierdzenie" w wątku przenosi do Broker-UI; kolor ryzyka (niskie/średnie/wysokie) w kartach zatwierdzeń.

## Jak cofnąć

- Dodanie lub zmiana nazw poziomów to zmiana polityki Jądra — tylko przez Broker z potwierdzeniem właściciela.
- Rezygnacja z tokenów na rzecz statycznych ACL usuwałaby „potomek ≤ rodzic" i TTL; wymaga rewizji `THREAT_MODEL.md` i ADR 3.
