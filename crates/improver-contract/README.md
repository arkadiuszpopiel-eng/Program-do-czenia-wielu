# improver-contract

Kontrakt Ulepszacza (`docs/modules/improver/SPEC.md`, PLAN §12.1, §12.4): pierścienie zmian (`Ring`: R0 prompty,
słownik wymowy, wagi routera, ustawienia niekrytyczne; R1 umiejętności i manifesty agentek; R2 wtyczki; R3 kod —
tylko szkic zgłoszenia; Jądro — nigdy), strażnik granic (`assess`: lista dozwolonych kluczy z typem, zakresem i
kierunkiem „zawężania”, zakaz `kernel.*`, prywatności, budżetów, uprawnień, autonomii, egressu, progów i zestawów
`evals`, własnych ustawień Ulepszacza; brak portu zapisu plików), typy propozycji i etapów, porty (`Proposer`,
`ApprovalVerifier`, `ImproverHost`) oraz rdzeń `ImproverCore`: obserwacja → propozycja → piaskownica (`test`) →
holdout (bramka Jądra, wynik zbiorczy) → wdrożenie (auto tylko R0 zawężające/bezpieczne, reszta po zatwierdzeniu
użytkownika) przez `ConfigStore` z `Origin::Improver` → nadzór i automatyczny rollback przy regresji.
