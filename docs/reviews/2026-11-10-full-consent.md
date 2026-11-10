# Pełna zgoda — decyzja właściciela i instrukcja instalowania (#74)

Baza `f642285585b0926566929fcd39164a7be715a795`: CI18110/10 i Windows1044/4 zakończone
sukcesem przed rozpoczęciem przyrostu. Nowe polecenie właściciela10.11 potwierdza maksymalne
dostępne możliwości i uprawnienia po świadomym włączeniu pełnej zgody działania.

## Zmiana

Runtime dołączał do informacji o programach bezwarunkową instrukcję: „brakujące zaproponuj
do instalacji właścicielowi”. To mogło wywoływać ponowne pytanie mimo aktywnej zgody.
Zastąpiono ją wykonaniem instalacji potrzebnej do zleconego zadania dostępnymi narzędziami
w ramach bieżących zgód. Agentka ma sprawdzić wynik i zgłosić faktyczną odmowę lub wymagane
potwierdzenie. Decyzja zapisana w [planie50](../plan/50-tryby-zgod-jak-claude-codex.md).

To zmiana instrukcji modelu, nie nowe nadanie praw. Istniejące Pełny/Max dostęp nadal
rozstrzygają zgody; nie zmieniono Brokera, zakresów ról, UAC ani STOP. Tryb Plan pozostaje
tylko do odczytu. Włączenie trybu w programie nie dowodzi uzyskania administratoraWindows.

## Walidacja

- Runtime contract/impl/fake, wszystkie funkcje: **132/0/0**, 29wyników celów.
- Clippy tych trzech crate'ów, wszystkie cele/funkcje, `-D warnings`: **kod0**.
- Istniejące testy `safety-broker-impl/max_access` i `standing_level`: **6/0/0** — lokalnie,
  chmura/wszystkie, zachowanie po restarcie, wyłączenie/odmowa i aktywacja przez właściciela.
- Istniejący kontrakt `autonomy_table`: **3/0/0** — zakresy, pierwszeństwo i wygasanie.
- Format, lint (Svelte/CSS/rozmiary/vendor), kontrola103generowanych stronHTML i diff: **kod0**.
- Komendy i SHA256 logów: [artefakt](artifacts/full-consent-validation.json).

Nie dodawano testu kopiującego słowa promptu. Nie ma nowego pomiaru zachowania LLM po tej
zmianie, rzeczywistej instalacji programu/sterownika ani elewacji UAC. Przebiegi uprawnień
działają na silniku/kontraktach w Linux; nie są odbiorem Windows, GPU lub mikrofonu.
Pełny CI kolejnego heada należy sprawdzić w [PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24).
Wyników poprzednika nie przypisujemy nowej zmianie. Bez merge; niezależny przegląd pozostaje otwarty.

Surowe logi: `/workspace/scratch/full-consent`. Chroniona łatka testu prywatności Core jest
osobnym przedmiotem przeglądu; decyzja o zachowaniu pełnej zgody nie modyfikuje tego testu.
