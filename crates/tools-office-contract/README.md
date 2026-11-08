# tools-office-contract

tools-office: kontrakt narzędzi Word/Excel dla agentek — odczyt tekstu, tabel i zakresów (treść niezaufana), edycja kopii roboczej z zapisem nowej wersji przez dziennik cofania (oryginał nietknięty), makra zawsze wyłączone, formuły z listy dozwolonej.

Kontrakt modułu `tools-office`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/tools-office/SPEC.md`](../../docs/modules/tools-office/SPEC.md).

```bash
cargo test -p tools-office-contract
```
