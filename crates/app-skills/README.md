# app-skills

Umiejętności i Kreator agentek w aplikacji (kategoria `app-*`).

- `SkillsApp` (`skills-impl`, katalog `%LOCALAPPDATA%\Alfa\skills`) — biblioteka (stan, wersja,
  źródło, kwarantanna), `skills_review` (diff względem zainstalowanej wersji + hash), instalacja
  i zwolnienie z kwarantanny **tylko z UI** z hashem przejrzanej wersji, `skills_run` = zadanie
  agentki (`TasksApp::create_skill`, parametry walidowane schematem, koperta ≤ roli), eksport
  i import paczki (import z zewnątrz → kwarantanna); most `skills.*` → `SkillsChanged`.
- `BuilderApp` (`agent-builder-impl`, `%LOCALAPPDATA%\Alfa\agents`) — szkic z rozmowy (`builder_propose`)
  albo formularza, podgląd persony (odmiana imienia, kolor, głos v0, rola, narzędzia, limity, hash),
  test na sucho (decyzje polityki dla scenariusza), zapis tylko po zaliczonym teście tego samego
  hasha i nie głosem; sufit autonomii z Brokera (nigdy L4), odsłuch głosu v0 (mówczyni bazowa).
- `open(WorkDeps)` — oba moduły; zdrowie w rejestrze przez `Work::health`.

Testy: `crates/app-core/tests/computer.rs` (propozycja → przegląd → zatwierdzenie hashem → zadanie;
zapis Kreatora dopiero po teście na sucho), `src/diff.rs` (diff linii).
