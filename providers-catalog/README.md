# providers-catalog — deklaratywny katalog dostawców (PLAN.md §5.6)

Jeden plik `<provider>.toml` na dostawcę. Katalog czyta `accounts-hub` (kreator „Dodaj dostawcę / konto / klucz") i `router` (tagi prywatności/jurysdykcji, możliwości). Nowy dostawca zgodny z OpenAI/Anthropic = nowy wpis + adapter generyczny, **bez zmian w kodzie**. Schemat wpisu: `schema.json` (JSON Schema; walidacja w CI po konwersji TOML → JSON).

Stan: **szkice** z 30.09.2026. Wartości, których plan nie potwierdza, mają `"unknown"` lub `"TODO"`; przed użyciem wpis przechodzi weryfikację jak trasa (`docs/compliance/subscription-routes.md` §6).

## Pola
| Pole | Typ | Znaczenie |
|---|---|---|
| `id` | string | identyfikator = nazwa pliku |
| `display_name` | string | nazwa w UI |
| `kind` | `chat` / `stt` / `tts` / `multi` | rodzaj usług |
| `auth` | `api_key` / `oauth_cli` / `none` | sposób uwierzytelnienia; klucze tylko w Windows Credential Manager |
| `base_url` | string | endpoint; tylko jeśli publicznie znany **i** wymieniony w planie, inaczej `"TODO"` (użytkownik/kreator uzupełnia) |
| `compat` | `openai` / `anthropic` / `native` | który adapter: generyczny OpenAI-compat, generyczny Anthropic-compat, natywny |
| `capabilities` | tabela | `vision`, `tools`, `streaming`, `long_context` — `true`/`false`/`"unknown"` |
| `privacy_tag` | string | tag z rejestru zgodności (`google-personal-may-train`, `google-paid-eea-no-train`, `xai-retention-30d`, `cn-may-train`, `sg`, `eu`, `unknown`) |
| `jurisdiction` | string | kod (`CN`, `SG`, `EU`, `US`…) lub `"unknown"` |
| `[pricing]` | tabela | **pusta w katalogu** — cennik uzupełniany w konfiguracji użytkownika (`%APPDATA%\Alfa\config`), nie w kodzie; Router liczy koszt z konfiguracji (§5.5) |
| `terms_url` | string | link do regulaminu lub `"TODO"` |
| `compliance_status` | `green` / `gray` / `forbidden` / `unverified` | status API-dostawcy (nie mostu CLI; mosty są w rejestrze zgodności) |
| `notes` | string | uwagi, w tym oznaczenia [V]/[W]/[?] |

Zasady:
- `"unknown"` jest wartością legalną i oznacza „nie wiemy" — Router traktuje ją ostrożnie (jak brak możliwości / „może trenować").
- Modele i ich identyfikatory **nie są** w katalogu: wykrywane automatycznie (Models API) po dodaniu klucza (§5.6).
- Zmiana `privacy_tag` lub `compliance_status` to zmiana polityki Jądra — tylko przez Broker, nie przez Ulepszacz (§12.4).
- Walidacja lokalna: `python3 -c "import tomllib,glob;[tomllib.load(open(f,'rb')) for f in glob.glob('providers-catalog/*.toml')]"`.

## Wpisy
anthropic, openai, google, xai, deepseek, kimi, qwen, zai, minimax, openrouter, mistral, elevenlabs, cartesia, azure-speech, soniox, custom-openai-compatible, custom-anthropic-compatible.
