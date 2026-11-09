# Router — anulowanie i współpraca lokalnie/API, 9.11.2026

Karta[#54](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/54),
baza`e3f70a8b24083e00ca2b724239226ca6b5004a17`, osobny worktree`/workspace/wt-enterprise-router`.
Wznowione przez właściciela naprawy i ulepszenia: [plan55](../plan/55-enterprise-lokalnie-chmura-i-wyszukiwanie.md).
Nie jest to deklaracja ukończonej jakości Enterprise całej aplikacji.

## Problem i naprawa

Router przekazywał token anulowania do adaptera, ale podczas oczekiwania na zdarzenie sam
wybierał tylko `tx.closed()` albo odpowiedź adaptera. Adapter ignorujący token pozostawiał
trasę w oczekiwaniu po Stop. Dotyczyło to także sytuacji po Started lub częściowym tekście,
gdy termin pierwszego zdarzenia już nie działa, a także jedynego/ostatniego dostawcy.

Teraz pętla próby niezależnie obserwuje anulowanie. Raportuje`Outcome::Cancelled`, emituje
jeden`Stop(Cancelled)` i zwalnia strumień adaptera; próba half-open zostaje zwolniona bez
naliczenia awarii. Nie uruchamia zapasu po Stop i nie odtwarza rozpoczętej treści lub narzędzia.
Zmiana produkcyjna to sześć linii w`router-impl/src/fallback.rs`.

Nie zmieniono polityki Routera, progów evals, limitów czasu, możliwości modeli, uprawnień,
licencji ani chronionych modułów. Router może zwolnić strumień i zakończyć własną trasę;
nie potwierdza zatrzymania zewnętrznego serwera, który ignoruje również zerwane połączenie.

## Odtworzenie i regresje

Testowy dostawca celowo ignoruje token i po opcjonalnym prefiksie nigdy więcej nie odpowiada.
Sygnał Notify potwierdza wejście w oczekiwanie przed Stop; bez przypadkowego sleep.

| Przebieg | Wynik |
| --- | --- |
| Przed poprawką: trzy przypadki anulowania | **0 zaliczonych/3 nieudane**, kod101; przekroczony100ms watchdog zegara wirtualnego |
| Pierwsze GREEN po poprawce | 4/4, w tym zwolnienie half-open |
| Końcowe cancel_stalled | **5/5**,12 wariantów lokalnie/API: przed zdarzeniem,po Started,tekst,narzędzie,half-open; z zapasem i bez |
| Nowe hybrid_tools | **3/3**,6 wariantów: lokalny→API,API→lokalny i samodzielne; cały request wraz z identyfikatorami i wynikami narzędzi; brak zapasu po rozpoczęciu wywołania |
| Pełne router-contract/router-impl/router-fake,all-features | **58 zaliczonych/0 nieudanych/0 ignorowanych**,15 wyników celów łącznie z pustymi doctestami |
| Clippy tych trzech crate'ów,all-targets/all-features | kod0, bez ostrzeżeń,18,31s |
| Format,lint,rozmiary,integralność vendora | kod0;0 naruszeń,29 plików vendora zgodnych |
| Generowanie,--check,--verify dokumentacji | 102 strony,0 problemów |

Pełny zestaw obejmuje też dotychczasowe kontrakty i regresje: prywatność/jurysdykcja, budżet,
odcięty dostawca, 429/5xx, brak modelu/narzędzi, kontekst większy od okna, historia i podpisy
rozumowania, zmiana dostępności konta oraz brak maskowania błędu po częściowej odpowiedzi.
Nie przydzielamy tym58 testom statusu rzeczywistej rozmowy z płatnym API lub wykonania narzędzia.

Komendy (Rust1.94.1, brak równoległego rzeczywistego LLM lub czasowych prób UI):

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p router-contract -p router-impl -p router-fake --all-features -j2
cargo +1.94.1 clippy --locked -p router-contract -p router-impl -p router-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
```

Zapis maszynowy i odciski logów: [validation.json](artifacts/enterprise-router-validation.json).
Logi lokalne: `/workspace/scratch/enterprise-router/`; historia RED pozostaje zachowana.
Testy wykonano na Linux Cloud z atrapami dostawców i wirtualnym zegarem przypadków awaryjnych.
Dotychczasowy pomiar fallbacku/anulowania na zegarze rzeczywistym jest osobnym testem w tym samym
zestawie. Brak odbioru RTX2060 12GB, prawdziwego mikrofonu, płatnego API i Windows tej poprawki.

## Co pozostaje

- Odpowiednie CI nowego commitu oraz świeży niezależny przegląd przed scaleniem; PR pozostaje draft.
- #41/#48: jakość tekstu person nadal otwarta; ta zmiana nie naprawia samych wag modelu.
- Kompletny dostawca wyszukiwania WWW i jego odbiór jakości nie są jeszcze zaimplementowane.
  Port SearchPort i atrapa nie wystarczają do deklaracji działającego wyszukiwania.
- Kolejne przyrosty według planu55: fakty wykonania narzędzi, produkcyjny dostawca wyszukiwania,
  cytowania i jakość źródeł, następnie odbiór systemowy lokalnie/chmura/hybryda i GPU.
