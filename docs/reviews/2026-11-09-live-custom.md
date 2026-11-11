# Własna Luna na rzeczywistym Bieliku, #43

Aktualizacja9.11: poniżej jest historyczna próba z ręcznym przypisaniem roli.
Nowa próba `custom_persona_on_local_model_after_catalog_and_cast_restart` odtwarza obsadę
bez takiego przypisania; dawny filtr nazwy poniżej jest archiwalny.
[Nowy raport, dowody i nadal otwarte problemy jakości](2026-11-09-jarvis-model-repair.md).

9.11.2026, Cloud Linux CPU, headless Core, syntetyczne audio. **10/10 tur ukończonych w36,99s**,
0 błędów,0 obcięć, autor `luna` w10/10. Jarvis dziedziczony z ustawienia wspólnego działa także
po restarcie katalogu i ponownym przypisaniu roli. Nie oznacza to trwałej obsady rozmowy (#37).

## Zakres

Nowy, oddzielny, jawnie pomijany w zwykłym CI test
`custom_persona_on_local_model_after_catalog_restart` w `jarvis_live.rs`.
Dotychczasowy protokół40 tur person wbudowanych pozostał bez zmiany (wyłącznie168 nowych linii).
Publiczny Kreator: podgląd → test na sucho tego samego hasha → zapis Luny z rolą Rozmówczyni.
Rola ma pustą listę narzędzi; model zachowuje `tools=false`. Każda z10 tur ma nową rozmowę
i jawne przypisanie roli. Po5 turach Standard zapisano wspólny Jarvis, odtworzono Core na tych
samych danych i sejfie testowym, następnie przypisano rolę w5 nowych rozmowach.

Bielik1.5B v3 Q8_0 i llama.cpp b11559, te same sprawdzone pliki;96 tokenów, temperatura0,
120s na turę,4 wątki CPU. Model działał bez konkurujących kompilacji i testów UI.
Po próbie serwer został zatrzymany; odczyt procesów nie wykazał aktywnego llama-server/Cargo.

| Profil | Odpowiedź,5/5 identycznych | Imię | Jedno zdanie |
| --- | --- | --- | --- |
| Standard | „Luna, jestem gotowa do rozmowy i pomocy.” |5/5 |5/5 |
| Jarvis po restarcie | „Jestem Luną. Jestem gotowa do pomocy.” |5/5 |0/5 |

Jakość Jarvisa pozostaje częściowa. To potwierdzenie integracji i tożsamości jednej własnej
persony, bez zaliczenia żądanej długości. Oba wcześniejsze eksperymenty instrukcji #41 cofnięto.

## Sprawdzenie i dowody

- [Surowy raport10 tur](artifacts/jarvis-bielik-luna-2026-11-09.json): tekst, autor, obcięcie,
  użycie tokenów, restart, ponowne przypisanie ról oraz pełne `router_request_system`.
  Instrukcje pochodzą z publicznego syntetycznego draftu; to żądanie routera do adaptera,
  nie zapis surowej transmisji HTTP. Liczba narzędzi0 w10/10.
- Test sprawdza niezmienione uprawnienia globalne i stan głosu różny od aktywnego.
  To nie pomiar prawdziwego mikrofonu, odsłuch ani wykonanie narzędzi.
- Pierwsza kompilacja zakończyła się E0308: helper deklarował `Sidecar`, konstruktor zwraca
  `Arc<Sidecar>`. Typ poprawiono; kolejna kompilacja przeszła w13,26s. Oba logi zachowane:
  `/workspace/scratch/live-custom-build.log` i `live-custom-build-fixed.log`.
- Clippy `-p app-core --test jarvis_live -D warnings`:0 ostrzeżeń,2m21s;
  format i rozmiary351 linii pliku,3487 plików/264 crate'ów/0 naruszeń.
- Komenda rzeczywistej próby (z `ALFA_LIVE_*` wskazującymi przypięte pliki i raport):

```sh
cargo test --locked -p app-core --test jarvis_live -j2 \
  custom_persona_on_local_model_after_catalog_restart -- --ignored --nocapture
```

Log `/workspace/scratch/live-custom-luna.log`:1 test zaliczony,1 stary test odfiltrowany.
Pełny wcześniejszy Rust3492/0/24 na169a6004 nie zawierał tego nowego testu ręcznego;
pozostałe źródła produkcyjne Rust są takie same. [Pełne wyniki platform](2026-11-09-full-validation.md).
UI osobnych profili obejmuje cztery persony wbudowane; własne dziedziczą profil wspólny.
Przegląd, usługa Windows na osobnym koncie i sprzęt właściciela pozostają odrębne.
