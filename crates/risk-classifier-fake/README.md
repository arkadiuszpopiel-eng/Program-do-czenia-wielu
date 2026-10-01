# risk-classifier-fake

Atrapa klasyfikatora: domyślnie ta sama tabela co `-impl`, opcjonalny skrypt werdyktów per narzędzie
(`script(tool, level, verdict)`) i rejestr wywołań. Twardych blokad Jądra nie da się zaskryptować —
fakty z `kernel_rule` zawsze dają `HardBlock`, żeby atrapa nie uczyła innych modułów błędnych założeń.
Przechodzi współdzielony test kontraktowy.
