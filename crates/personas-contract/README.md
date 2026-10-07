# personas-contract

Kontrakt modułu `personas` (docs/modules/personas/SPEC.md, docs/PERSONAS.md, PLAN §9.2).

- **Persony** (stała tożsamość, zawsze żeńska): Alfa/Beta/Gama/Delta — imię, glif α β γ δ, **nazwa**
  tokenu koloru z `packages/ui-kit` (`color.agent.<id>`, nigdy wartość hex), charakter, frazy
  wywoławcze („Hej Alfa”), odmiana imienia przez przypadki (`NameForms`: Delta/Delty/Delcie/Deltę/
  Deltą/Delcie/Delto), **biblia głosu jako dane** (`VoiceBible`: wiek postrzegany 18–25, barwa,
  rejestr, tempo, energia, emocje, prompt voice design bez „girl/cute/child”, pochodzenie i zgoda).
- **Role** (9 wbudowanych + własne): Dyrygentka, Mówczyni, Myślicielka/Planistka, Wykonawczyni/
  Operatorka, Koderka, Krytyczka/Weryfikatorka (tylko odczyt), Badaczka (izolacja), Strażniczka
  pamięci/organizacji, Pisarka/Tłumaczka — z promptem (rodzaj żeński), polityką modelu i narzędziami.
- **Obsada** (`Cast`: persona → zbiór ról, per sesja) i **szablony** Standard / Solo / Kodowanie /
  Badania / własne. Walidacja (`Catalog::validate_cast`): dokładnie jedna Dyrygentka, role unikalne
  najwyżej raz, w sesji głosowej dokładnie jedna Mówczyni; ostrzeżenia: brak Krytyczki, Krytyczka
  z rolą autorki. „Krytyczka ≠ autorka, gdy możliwe” → `Cast::verifier_for(autorka)`:
  Krytyczka ≠ autorka, inaczej zastępczyni (Myślicielka → Dyrygentka → ktokolwiek), a samoweryfikacja
  tylko gdy autorka gra sama (Solo).
- **Adresatka** (`parse_addressee`, `resolve_addressee`): `@beta`, „Beta, …”, „hej Gama”, „Delto”,
  „…, Delta?”; imię zawsze wygrywa, bez imienia → Dyrygentka.
- **Polecenia obsady** (`parse_cast_command` + `apply_command`): deterministyczny parser PL
  („Beta, teraz ty prowadzisz”, „Delta, przejmij weryfikację”, „Przekaż prowadzenie Delcie”,
  „Gama, przestań weryfikować”, „Beta, ty też kodujesz”, „Obsada solo”, „Alfa, zrób wszystko sama”);
  pytania i zwykłe prośby („Delta, sprawdź pogodę”) nie są poleceniami.
- **Prompt systemowy** (`render_system_prompt`): szablon z `{imie} {glif} {charakter} {role}
  {opis_rol} {zasady}` + zasady wspólne (jedna mówi naraz, bez samodzielnej zmiany obsady…).
- Trait `Personas`, zdarzenia `personas.cast.changed`, `personas.role.assigned`,
  `personas.persona.added` (`CastChange::events`), eksport `.alfa` (`PersonasExport`, serde + JSON Schema).

Logika jest czysta i deterministyczna; `PersonasState` to rdzeń stanu dzielony przez `-impl` i `-fake`
(różnią się tylko ujściem zdarzeń), więc oba przechodzą ten sam test kontraktowy (`contract-tests`).
Budżet `resolve_addressee` ≤ 1 ms — to kilka przebiegów po słowach wypowiedzi.

Testy: tabela 44 poleceń PL (34 polecenia + 10 nie-poleceń), tabela 22 zwrotów, własności
(1000 przypadków: imię zawsze wygrywa; bez imienia → Dyrygentka; dowolny ciąg poleceń zachowuje
poprawność obsady; parser totalny), katalog i Kreator (walidacja, kolizje form imienia i glifów,
eksport/import), prompty bez form męskich.
