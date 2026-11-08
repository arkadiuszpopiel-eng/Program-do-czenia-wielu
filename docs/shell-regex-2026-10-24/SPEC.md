# Fałszywy egress z argumentu regex

Literalny wzorzec porównania PowerShell, np. -notmatch '\\(obj|bin)\\', nie jest operacją na udziale UNC ani zewnętrzną ścieżką. Usunąć fałszywy host i żądanie net.egress dla tego argumentu. Faktyczne UNC, URL, wysyłki i ostrożna obsługa interpolacji pozostają. Nie zmieniać Brokera, polityki L4, taintu ani progów zgód. Test narzędzia z rzeczywistym silnikiem Brokera w L4 i tainted session ma wykonać lokalne polecenie bez nowej zgody oraz nadal odmówić rzeczywistej wysyłki bez potwierdzenia.
