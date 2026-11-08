# ADR 0005 — Dwa kontrakty: `ModelProvider` (tokeny) i `AgentBackend` (zadania)

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.3, §5.1–5.6, §8.5, §9.7, §16.2 (F1, F4) |

## Kontekst

Alfa łączy cztery klasy „mózgów": lokalne modele (llama.cpp), chmurowe API (Anthropic, OpenAI, adapter generyczny), mosty do oficjalnych CLI na planach abonamentowych (Claude Code, Codex, potem Grok Build, Kimi Code, `agy`) oraz własne endpointy. Klasy A/B/D dają strumień tokenów; klasa C to proces, który sam planuje, prosi o uprawnienia, wznawia sesje i zwraca zdarzenia. Regulaminy (§1.3) pozwalają sterować niezmodyfikowanym CLI, do którego loguje się użytkownik, ale zabraniają czytania tokenów i udawania `chat.completions`.

## Decyzja

| Kontrakt | Jednostka | Metody | Klasy |
|---|---|---|---|
| **`ModelProvider`** | tokeny | `capabilities()`, `stream(req)` z anulowaniem, `health()`, `cost()` | A. lokalne, B. chmura API, D. własne |
| **`AgentBackend`** | zadania | `submit_task`, `events`, `approve`, `steer`, `cancel`, `resume` | C. mosty CLI („opaque worker"), D. własne |

- Rodzaje `ModelProvider`: chat, STT, TTS, embeddings, wizja/OCR, S2S.
- **Router kieruje zadania**, nie żądania tokenowe: klasa zadania (głos-szybka, rozmowa, kod, planowanie, GUI/wizja, streszczanie, embeddingi) × ograniczenia (tag prywatności i jurysdykcji, budżet, opóźnienie, możliwości); fallback, circuit breaker, reaktywne wykrywanie limitów okien planów.
- Most = sterowanie **oficjalnym, niezmodyfikowanym CLI** przez jego tryby nieinteraktywne/SDK/app-server; natywne „ask" CLI przekierowane do naszego kanału (Claude Code: `--permission-prompt-tool`; Codex: approvals app-server — do potwierdzenia w spike (b)). Kod Alfy nigdy nie czyta ani nie przechowuje tokenów CLI.
- Mosty pracują w worktree/kopii; cofanie = snapshot przed/po; pamięć tylko przez `recall`; zdarzenia CLI w Audycie oznaczone „niezależnie niezweryfikowane"; zimny start (sekundy) → nigdy na ścieżce głosu.
- Adapter generyczny (od F1): `baseURL`, klucz, model ID, flagi możliwości, tag prywatności/jurysdykcji per profil.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Jeden kontrakt „chat completions" dla wszystkiego | mosty CLI nie są strumieniem tokenów; wymuszanie tego oznacza obejście regulaminów (surowe `chat.completions` przez plan) i utratę zdarzeń CLI (plan, uprawnienia, wznawianie) |
| Automatyzacja webowych UI dostawców | wprost wykluczone przez §1.3 (naruszenie regulaminów, kradzież ciasteczek/tokenów) |
| Mosty przez własną implementację protokołów dostawców | modyfikacja/udawanie klienta = naruszenie; przypięte wersje oficjalnych CLI zamiast tego |
| Router na poziomie tokenów | tagi prywatności, budżety i limity planów dotyczą zadań; fallback tokenowy w środku odpowiedzi psuje spójność |

## Konsekwencje

- Dwa osobne moduły: `providers-api`/`providers-local` (`ModelProvider`) i `agent-backends` (`AgentBackend`); `router` zna oba.
- Egzekucja techniczna: deny-lista ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) w `fs.*`/shellu; przypięte wersje CLI (nieznana wersja wyłącza trasę); osobne statusy „CLI `-p`" i „Agent SDK"; rejestr `compliance-registry.json` z wyłącznikiem trasy.
- Kryteria F4: 20/20 próśb o uprawnienia trafia do Broker-UI; monitor ETW: 0 odczytów `~/.claude`, `~/.codex`; wyłącznik: 0 wywołań wyłączonej trasy w 100 próbach.
- Testy chmurowe na fixture'ach syntetycznych ze schematów API do czasu dodania kluczy (`accounts-hub`).
- Serwer MCP Alfy wystawia mostom tylko narzędzia specyficzne dla Windows (F4: schowek, okna; F6: UIA, zrzuty, rejestr), bez fs/shell.

## Jak cofnąć

- Dodanie trzeciego kontraktu (np. dla natywnego S2S) nie łamie tej decyzji — S2S jest rodzajem `ModelProvider`.
- Gdyby regulamin danego dostawcy zmienił się na niekorzyść, trasa jest wyłączana w rejestrze zgodności bez zmian w kodzie; adaptery są izolowane.
- Scalenie kontraktów wymagałoby przepisania `router` i `agent-runtime`; nie przewidujemy tego.
