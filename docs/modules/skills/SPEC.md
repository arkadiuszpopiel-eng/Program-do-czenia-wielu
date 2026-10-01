# skills — SPEC (v1, zaimplementowany)

## Cel
Umiejętności agentek (PLAN §9.2 „agent podsystemu = paczka narzędzi + skill”, §9.5 „umiejętności = playbooki + narzędzia”, §12.1 pierścień R1, §8.0 „zatruta umiejętność”): nazwany, wersjonowany przepis, który agentka uruchamia przez `agent-runtime`, z uprawnieniami nigdy większymi niż jej rola.

## Fala i priorytet
F5, P1. Źródło „z pamięci” zasila F7 (konsolidacja → warstwa proceduralna); eksport w `.alfa` (F7 transfer pełny).

## Kontrakt (`skills-contract`)
```rust
pub struct Skill { id: SkillId, version: semver::Version, name, description, keywords, required_tools: Vec<String>, required_capabilities: Vec<String>,
                   parameters: JsonSchema /* obiekt zamknięty */, prompt /* {{param}} */, steps, examples, acceptance: Vec<AcceptanceTest>, budget: Option<RunBudget> }
pub enum SkillSource { Memory { entry, trusted }, User, Import { origin: OwnPackage | External } }
pub enum SkillState { Proposed, Quarantined, Installed, Rejected, Disabled, Superseded }
pub struct OwnerApproval { origin: Ui | Text | Voice, reviewed_hash }          // agentka nie ma wariantu
#[async_trait] pub trait Skills { catalog, list, installed, propose, approve, release /* kwarantanna, tylko Ui */, reject, disable,
    search(task, caller_roles, limit), prepare_run(id, params, caller: &RunSpec, &RunOptions, parent) -> (RunSpec, RunOptions), export, import, propose_from_memory }
pub fn search(..) / trait SkillRanker + rerank(..)                            // deterministycznie + port LLM
pub struct SkillBundle { format: "alfa.skills.v1", skills, sha256 }            // kanoniczny JSON, SHA-256
```
Zdarzenia: `skills.proposed|quarantined|installed|released|rejected|disabled|superseded|imported` — ładunek: id, wersja, hash, stan, liczba uwag (bez treści).

## Zasady
- **Walidacja przy propozycji:** pola i limity, schemat parametrów (podzbiór JSON Schema), znaczniki szablonu ⊆ parametry, narzędzia istnieją w katalogu (bez powtórzeń), zadeklarowane zdolności = zdolności z manifestów, zakaz `system.admin` i `secrets.read`, ≥ 1 test akceptacyjny i wszystkie przechodzą (deterministycznie: parametry → cel zawiera teksty / parametry odrzucone).
- **Instalacja i aktualizacja tylko po zatwierdzeniu właściciela:** `OwnerApproval.reviewed_hash` = hash wersji (podmiana treści po przeglądzie odrzucona); aktualizacja = wyższa wersja (ta sama wersja z inną treścią = błąd), starsza → `Superseded` (historia, możliwy powrót); import zawsze tworzy propozycje. Metody biblioteki nie są narzędziami agentek — wołają je wyłącznie komendy UI.
- **Kwarantanna:** źródło niezaufane (pamięć z niezaufanej proweniencji, import z zewnątrz) albo uwagi skanera (polecenia obejścia: „zignoruj…”, Broker/Jądro/audyt/autonomia, poświadczenia CLI, ciasteczka, `curl`/`Invoke-WebRequest`…) przy źródle innym niż właściciel; z kwarantanny nie da się uruchomić ani wyszukać; zwolnienie tylko `ApprovalOrigin::Ui` (nie głos, nie tekst).
- **Uprawnienia ≤ roli wywołującej:** każde wymagane narzędzie musi być dozwolone dla ról wywołującej (`ToolManifest::allowed_for`; brak ról = brak narzędzi); przebieg dostaje kopertę `RunGrant` = wymagania ∩ koperta wywołującej (narzędzie utracone przy atenuacji = odmowa), budżet ∩, pochodzenie, taint i proweniencja wywołującej; Broker i tak decyduje per akcja.
- **Wyszukiwanie:** tylko zainstalowane i uruchamialne dla roli; rdzenie słów po `fold` (5 znaków — odmiana PL), wagi: nazwa/słowa kluczowe 3, opis 2, szablon/kroki 1, próg 0,2; model (`SkillRanker`) przestawia ≤ 8 kandydatów, nie może dodać innych, oceny przycinane, błąd modelu = kolejność deterministyczna.
- **Z pamięci:** `draft_from_memory` (warstwa proceduralna): tytuł, kroki z listy, narzędzia wspomniane z nazwy, jeden test akceptacyjny; dalej zwykła ścieżka propozycji.

## Eksport / import (`.alfa`)
`SkillsDocuments` (`skills-impl`) = `DocumentStore` kategorii `skills`: dokument `skills.json` = `SkillBundle` zainstalowanych umiejętności (kanoniczny JSON, SHA-256). Zapis = weryfikacja hasha + import jako propozycje z `OwnPackage` (nigdy instalacja); usunięcie dokumentu niczego nie kasuje. Paczka z innego źródła (`External`) → kwarantanna.

## Zależności
`agent-runtime`, `tools-common`, `personas`, `memory`, `risk-classifier` (`-contract`); `skills-impl` dodatkowo `transfer-contract`, `core-registry-contract`.

## Izolacja / budżet
`inproc`, `lazy`; RAM ≤ 2 MB; wyszukiwanie ≤ 5 ms dla 1000 umiejętności (liniowo, bez indeksu).

## Integracja (`app-*`, opis)
`SkillsModule::new(katalog manifestów narzędzi, DirSkillStore::open(%LOCALAPPDATA%\Alfa\skills))` w rejestrze; komendy UI `skills_list/propose/approve/release/reject/disable/export/import`; `SkillRunner` z runtime agentek (przebieg z obsady wywołującej); `SkillsDocuments` w `TransferPorts.documents[Category::Skills]`; konsolidacja pamięci: wpisy `Derivation::Skill` → `propose_from_memory` (karta „nowa umiejętność do przejrzenia”).

## Testy akceptacyjne
- Kontrakt (`contract_tests`: cykl życia, kwarantanna, eksport/import) na `-impl` i `-fake`.
- `skills-contract/tests/library.rs`: 11 przepisów niepoprawnych odrzuconych (m.in. zdolności Jądra), zdarzenia bez treści, szkic z pamięci (zaufany/niezaufany), wyszukiwanie i model-przeciwnik (nie wprowadza obcych), własności: „uruchamialna ⇔ narzędzia ⊆ rola”, hash niezależny od kolejności kluczy.
- `skills-impl/tests/module.rs`: trwałość, błąd zapisu = brak zmiany, zdarzenia, runner (koperta, odmowa roli), dokument `.alfa` (import ≠ instalacja, zmieniony bajt = odrzucenie).

## Fake
`skills-fake`: ten sam rdzeń, magazyn w pamięci, wirtualny zegar, nagrane zdarzenia.

## Otwarte pytania
- Podpis zatwierdzeń R1 kluczem w TPM (PLAN §12.1) — dziś hash przejrzanej treści + kanał UI.
- Szyfrowanie `skills.json` (ścieżki użytkownika w parametrach przykładów).
