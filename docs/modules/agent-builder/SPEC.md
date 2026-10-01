# agent-builder — SPEC (v1, zaimplementowany)

## Cel
Kreator agentów (PLAN §9.5, §9.2 „więcej agentek dodajesz Kreatorem”): opis słowami albo formularz → **manifest** (persona + rola + głos v0 + limity) → podgląd i **test na sucho** → **zapis po zatwierdzeniu** właściciela do katalogu `personas` i biblioteki. Pierścień R1 (§12.1): zmiana po testach, przegląd przez właściciela. Kreator **nie tworzy ról z uprawnieniami Jądra i nie podnosi autonomii**.

## Fala i priorytet
F5, P1.

## Kontrakt (`agent-builder-contract`)
```rust
pub struct AgentDraft { id?, name?, forms?: NameForms, glyph?, color?, character?, voice?: VoiceDraft{base, pitch, rate, perceived_age, timbre, design_prompt},
                        role?: RoleDraft{id, name, description, prompt, model_policy, tools /* grupy */, read_only, untrusted_isolated, author}, limits: LimitsDraft, skills }
pub struct AgentManifest { persona: Persona, role: Role, voice: VoicePreset, limits: AgentLimits{autonomy, budget, fs_write, memory_scope, retain_days, triggers}, skills }
pub struct BuilderPolicy { allowed_groups, ceiling: AutonomyLevel /* ≤ L3 */, max_budget, palette }
pub fn from_description(&str) -> DraftProposal{draft, questions}; trait DraftLlm; async fn from_conversation(&str, &dyn DraftLlm)
pub fn build_manifest(&AgentDraft, &BuilderPolicy, &BuildContext) -> Result<Built{manifest, hash, warnings}, BuildError>
pub fn preview(..) -> Preview; pub fn dry_run(&AgentManifest, &[ToolManifest], &DryScenario) -> DryRunReport
pub struct BuilderApproval { origin: Ui | Text | Voice, reviewed_hash }
#[async_trait] pub trait AgentBuilder { policy, propose, propose_with, build, preview, async dry_run, async save, library }
```
Zdarzenia: `agent_builder.dry_run` (hash, wynik), `agent_builder.saved` (persona, rola, hash, autonomia, grupy narzędzi) + `personas.persona.added`.

## Reguły walidacji
- **Persona:** imię (litery, wielka pierwsza, ≤ 32, bez słów zakazanych), identyfikator z imienia, odmiana przez przypadki (`decline_feminine`: zmiękczenia t→cie, d→dzie, r→rze, k→ce, g→dze…, rdzenie miękkie i zdrobnienia „-sia/-nia”), glif, kolor wyłącznie z palety `color.agent.custom-1..8`, charakter, biblia głosu (wiek 18–25, prompt voice design bez „girl/cute/child”), fraza „Hej {Imię}”; kolizje id/imienia/form/glifu z katalogiem → odrzucenie.
- **Głos v0:** mówczyni bazowa wbudowana (`pl-f1`, `pl-f2`, zapas Piper), wysokość 0,7–1,4, tempo 0,6–1,6, brzmienie odrębne od wszystkich istniejących głosów (`validate_chains`); bez podania — pierwszy wolny preset.
- **Rola:** identyfikator i nazwa bez znaczników Jądra; prompt ≤ 4000 znaków, **w rodzaju żeńskim** (cały prompt systemowy sprawdzony), bez fraz obejścia (podnoszenie autonomii/L4, wyłączanie audytu, omijanie Brokera, samozatwierdzanie, Jądro, hasła, poświadczenia CLI, ciasteczka, `sudo`/`root`/`admin`); klasa modelu z listy; **grupy narzędzi wyłącznie z listy dozwolonych** (grupy ról wbudowanych, ASCII, małe litery) — znaczniki Jądra (`kernel`, `broker`, `audit`, `autonom`, `policy`, `secret`, `admin`, `egress`, `approv`, `token`, `system.`…) dają `KernelPermission`, reszta spoza listy `ForbiddenGroup`; źródła zewnętrzne → `untrusted_isolated`.
- **Limity:** autonomia ≤ sufit (= poziom sesji tworzącej z Brokera, **nigdy L4** — L4 tylko przełącznikiem w Ustawieniach; w Brokerze stosowana wyłącznie jako obniżenie); budżet ≤ sufit; zakresy zapisu tylko w profilu (`%USERPROFILE%\`, `~/`, `C:\Users\<konto>\`), bez `..`, UNC, katalogów Alfy/Brokera, systemu, `.ssh`, `.claude`, `.codex`, poświadczeń; pamięć `agent|session` (nie globalna), retencja 1–365 dni; wyzwalacze tylko jako dane (aktywuje moduł `triggers` po osobnym zatwierdzeniu).
- **Zapis:** manifest musi być dokładnie tym, co zbudowałby Kreator z jego pól (podmiana `builtin`, `unique`, autonomii, grup po budowie = odrzucenie), hash = przejrzany, zaliczony test na sucho tego hasha, kanał UI/tekst (głos — nie), potem `personas.add_role` → `add_persona` → biblioteka.

## Test na sucho
Skryptowany przebieg bez modelu i skutków: dla kroku (narzędzie + argumenty + oczekiwanie) — narzędzie spoza roli/katalogu = odmowa; zapis poza zakresem = odmowa; poziom L0 = odmowa zmian, L1 = pyta o każdą zmianę, L2 = pyta o nieodwracalne, L3 = sama. Zaliczony, gdy wszystkie kroki zgodne z oczekiwaniem.

## Rozmowa
Deterministycznie: imię po „agentkę/o imieniu…”, zadania po „która/żeby”, grupy ze słów (pliki/foldery → `fs`, Pobrane → zakres `Downloads\**`, polecenia → `shell`, kod → `worktree`+`shell`, strony → `web`+`browser`, notatki → `memory`, okna → `gui.control`; „tylko czyta” → `fs.read`, tylko odczyt), charakter, głos (niższy/wyższy/szybszy), autonomia (L4 → pytanie i odrzucenie przy budowie) + pytania o braki. Model (`DraftLlm`) uzupełnia tylko braki; autonomia = niższa z dwóch; wynik przechodzi tę samą walidację.

## Zależności
`personas`, `agent-runtime` (budżet), `tools-common`, `voice-tts`, `risk-classifier` (`-contract`); `-impl`: `core-registry-contract`.

## Izolacja / budżet
`inproc`, `on-demand`; RAM ≤ 2 MB; budowa ≤ 5 ms.

## Integracja (`app-*`, opis)
`AgentBuilderModule::new(personas, katalog manifestów narzędzi, DirManifestStore::open(%LOCALAPPDATA%\Alfa\agents), CeilingSource = |s| broker.autonomy(s, None))`; komendy UI `builder_propose/build/preview/dry_run/save`; po zapisie: obsada (`Personas::set_cast` na polecenie właściciela), poziom autonomii agentki w Brokerze przez `request_autonomy_change` tylko obniżająco, umiejętności przez `skills`, głos v0 do łańcucha `voice-tts` (`VoicePreset`).

## Testy akceptacyjne
- `ACC-F5-agent-builder-01`: ścieżka szczęśliwa (rozmowa → … → zapis) na `-impl` i `-fake`.
- `ACC-F5-agent-builder-02`: **41 prób ataku** (grupy Jądra, wielkie litery, homoglif, egress, L4, rola Jądra/wbudowana, 7 promptów obejścia, forma męska, 5 zakresów, budżet, imiona kolidujące/zakazane, pamięć globalna, głos „cute girl”/jak Alfa, 5 podmian manifestu po budowie) = **0 sukcesów** (`contract_tests::attacks`), każda z właściwego powodu (`tests/policy.rs`); własności (1000 przypadków): grupy tylko z listy, autonomia ≤ sufit, zakresy tylko w profilu.

## Fake
`agent-builder-fake`: ten sam rdzeń, katalog wbudowany, głosy v0, biblioteka w pamięci, nagrane zdarzenia.

## Otwarte pytania
- Usunięcie roli przy nieudanym `add_persona` (brak operacji usuwania w `Personas`).
- Piaskownica z modelem (przebieg `agent-runtime` na narzędziach skryptowanych) jako druga faza testu — port w `app-*`.
