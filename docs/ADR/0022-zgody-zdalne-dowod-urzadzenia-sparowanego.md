# ADR 0022 — Zgody zdalne: dowód urządzenia sparowanego (`PairedDevice`)

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na podpis właściciela |
| Data | 2026-10-09 |
| Powiązane | decyzja właściciela nr 2 z 2026-10-09; plan v6: `docs/plan/09-dostep-zdalny-i-bezpieczenstwo.md` (zgody zdalne, zabezpieczenia, R3), `03-roadmapa.md` (D.5, E.4); ADR 0021; `docs/modules/safety-broker/SPEC.md`, `docs/modules/broker-ui/SPEC.md` |
| Zmienia | ADR 0003 („zatwierdzenia tylko w Broker-UI, tylko z fizycznego wejścia”) i ADR 0015 (potwierdzenia wymagane przez poziomy L0–L4 mogą przyjść z urządzenia sparowanego). Obszar Jądra bezpieczeństwa — **wymaga przeglądu i akceptacji człowieka** (bramka #7) |

## Kontekst

Dziś zgodę przyjmuje wyłącznie okno Brokera i wymaga `PhysicalInputProof`: dowodu, że kliknięcie pochodzi
od człowieka przy komputerze (albo Windows Hello). Interfejs w WebView nigdy nie zatwierdza. Przy dostępie
zdalnym zadania agentek stałyby w miejscu, gdy właściciela nie ma przy komputerze. Właściciel zdecydował
(decyzja 2), że zdalnie wolno zatwierdzać **wszystkie** działania, także wysokiego ryzyka. To wygodne,
ale przejęty odblokowany telefon staje się wtedy kluczem do komputera — projekt musi to ograniczyć,
nie odbierając wygody.

## Decyzja

Dochodzi drugi rodzaj dowodu; pierwszy nie jest osłabiany. Dowód od urządzenia odwołanego jest odrzucany.

```rust
// szkic — kontrakt należy do Jądra; zmiana tylko po przeglądzie właściciela
pub enum ApprovalProof {
    PhysicalInput(PhysicalInputProof),   // jak dziś
    PairedDevice(DeviceApprovalProof),   // nowe
}
pub struct DeviceApprovalProof {
    pub device_id: DeviceId,
    pub request_hash: [u8; 32],   // skrót dokładnie tego żądania: co, na czym, poziom ryzyka
    pub user_verified: bool,      // urządzenie potwierdziło biometrię / PIN przy TYM podpisie
    pub signed_at: Timestamp,
    pub signature: Signature,     // kluczem z bezpiecznego magazynu urządzenia
}
```

| Reguła | Propozycja |
|---|---|
| Wiązanie z żądaniem | podpis obejmuje skrót konkretnego żądania; nie da się go przenieść na inne działanie ani powtórzyć |
| Kto sprawdza | **Broker sam** sprawdza podpis kluczem z listy urządzeń; nie ufa bramie ani interfejsowi; brama klientów tylko przenosi dowód i nie może go wytworzyć |
| Weryfikacja użytkownika | `user_verified` musi być prawdziwe przy każdym podpisie: klucz w magazynie Androida wymaga biometrii przy każdym użyciu; w przeglądarce WebAuthn z `userVerification: required` |
| Co widzi właściciel | żądanie na telefonie zawiera to samo co okno Brokera: kto prosi, co, na czym, ryzyko, czy da się cofnąć |
| Dziennik | zgoda zdalna jest w Audycie oznaczona urządzeniem, z którego przyszła |
| Zakres | **wszystkie poziomy ryzyka** dopuszczone zdalnie (decyzja właściciela 2); twarde blokady Jądra pozostają zabronione dla każdego |

Zabezpieczenia przy „wszystko zdalnie”:

| Zabezpieczenie | Jak działa | Można wyłączyć? |
|---|---|---|
| Biometria przy każdej zgodzie | sam odblokowany ekran nie wystarcza | nie |
| Wysokie ryzyko: pełny opis i przytrzymanie przycisku | chroni przed przypadkowym dotknięciem | nie |
| Powiadomienie na komputerze i pozostałych urządzeniach o zgodzie wysokiego ryzyka | właściciel widzi, że coś zatwierdzono | tak |
| Limit zgód wysokiego ryzyka w czasie (np. 5 na 10 minut) | hamuje serię działań po przejęciu | tak (ustawienie) |
| Zakres per urządzenie | urządzeniu można odebrać prawo do wysokiego ryzyka | tak — domyślnie pełny, zgodnie z decyzją |
| Odwołanie urządzenia z dowolnego klienta | natychmiastowe | nie |
| Twarde blokady z modelu zagrożeń | to, czego Broker nie pozwala nikomu, pozostaje zabronione | nie |

**Rekomendacja sesji (do decyzji właściciela przy podpisie):** zostawić domyślnie pełny zakres, zgodnie
z decyzją 2, ale pokazać przy parowaniu przełącznik per urządzenie „to urządzenie może zatwierdzać wysokie
ryzyko” — domyślnie włączony.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Zdalnie tylko niskie i średnie ryzyko | sprzeczne z decyzją właściciela 2; pozostaje dostępne jako zakres per urządzenie |
| Zgoda zdalna zatwierdzana przez bramę albo interfejs („zaufany kanał”) | błąd w bramie lub XSS w panelu = samozatwierdzenie; dowód musi być sprawdzalny przez Brokera niezależnie od drogi |
| Podpis sesji zamiast podpisu żądania | jeden podpis otwierałby serię działań; nie chroni przed powtórzeniem |
| Biometria raz na sesję | odblokowany telefon w cudzych rękach zatwierdzałby dalej (R3) |

## Konsekwencje

- Zmiana kontraktu `safety-broker` i `broker-ui`: pakiet do przeglądu przygotowuje sesja, zatwierdza właściciel;
  kod dopiero po podpisie (D.0, D.5).
- ADR 0003 i 0015 dostają po podpisie dopisek o drugim rodzaju dowodu; reguła „destrukcja zlecona głosem
  wymaga potwierdzenia nie-głosem” pozostaje — dowód urządzenia jest potwierdzeniem nie-głosem.
- Otwarte: format podpisu w przeglądarce. Asercja WebAuthn podpisuje własną strukturę (dane uwierzytelniacza
  i dane klienta z wyzwaniem), a nie dowolny skrót — `DeviceApprovalProof` potrzebuje wariantu albo opakowania
  dla WebAuthn. Do ustalenia przed E.4. Okno ważności `signed_at` i wartości limitu zgód ustala właściciel.
- Testy wymagane przed włączeniem: powtórzenie dowodu i przeniesienie go na inne żądanie — odrzucone; dowód
  od urządzenia odwołanego — odrzucony; istniejące kryterium „agentka zatwierdza sama siebie = 0 sukcesów”
  rozszerzone o drogę zdalną.
- Uczciwie: bezpieczeństwo zgody zdalnej zależy od bezpiecznego magazynu i biometrii urządzenia, których Alfa
  nie kontroluje.

## Jak cofnąć

- Wariant `PairedDevice` można wyłączyć polityką Jądra (globalnie albo per urządzenie); zgody wracają do okna Brokera.
- Do podpisu tego ADR obowiązuje ADR 0003 bez zmian: zgoda tylko z fizycznego wejścia w oknie Brokera.
