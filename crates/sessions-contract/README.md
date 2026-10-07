# sessions-contract

Kontrakt modułu `sessions` (docs/modules/sessions/SPEC.md, ADR 0006/0008). Synchroniczny (SQLite blokuje;
z kodu async — `spawn_blocking`).

- `SessionCatalog` — tworzenie, metadane (tytuł, szablon, polityka modeli, agentki, tag prywatności, katalog
  roboczy, przypięcie, archiwum, kosz, projekt, tagi), `mark_tainted` (flaga tylko rośnie), lista z
  wyszukiwaniem bez diakrytyków i sortowaniem (`apply_query` — wspólne dla `-impl`/`-fake`), kropka
  aktywności, nieprzeczytane, kosz/przywrócenie, `delete_session` (crypto-shredding).
- `SessionHistory` — **drzewo append-only**: `append_turn` (tylko do liścia), `fork_from` (edytuj/ponów =
  wariant w nowej gałęzi), `branch_projection`, `siblings` (`‹ 1/3 ›`), `latest_leaf`, `set_active_leaf`,
  `record_heard_prefix` (fakt dopisywany raz), `set_hidden` (flaga widoku), szkic composera. **Brak
  operacji zmiany treści tury.**
- Typy tury: `Role`, `Author` (w tym `Handoff`), `TurnContent { text, blocks }` z blokami `Text`,
  `Thinking` (podpis dostawcy nieprzezroczysty), `RedactedThinking`, `ToolUse`, `ToolResult`, `Attachment`;
  `HeardPrefix { chars, approximate }` (tekst = `assistant_full`), `ModelUsage` (dostawca, model, `Cost`).
  `Turn::fingerprint()` = kanoniczne bajty niezmiennej części tury.
- `KeyVault` (+ `load_or_create_key`) — sejf kluczy; `SessionDbProvider` — dostęp innych modułów do tej samej
  szyfrowanej bazy sesji (`Arc<Db>`).
- `contract_tests` (feature `contract-tests`): 16 przypadków + `ops::check_ops` (property-based
  `ACC-F1-sessions-02`: dowolna sekwencja operacji → spójne drzewo, wcześniejsze tury bajtowo niezmienione).
