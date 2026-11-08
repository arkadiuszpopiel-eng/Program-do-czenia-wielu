---
name: alfa-pulpit-panele-2026-10-29
description: "Decyzja właściciela 29.10 — stan (profil/tryb/koszt) w pasku pisania, panele chowane w całości bez paska ikon, wysuwane i przypinane"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-29T09:07:03.214Z
---

Właściciel 29.10 (0.0.76) ocenił chowanie paneli jako „zrobione byle jak” i zgłosił, że przełączanie trybu pracy nie działa. Wybrał:

- ciemna kapsuła stanu z belki → żetony w pasku pisania (profil z kropką połączenia, tryb pracy, koszt); w belce tylko wskaźniki na żywo + bursztynowy znak przy L4;
- panele (Sesje, prawy, menu Ustawień) chowane w całości, bez paska ikon; przy krawędzi cienki świecący znacznik, najechanie wysuwa nad treść, „Przypnij panel”/klik znacznika dokuje; wszystko z płynną animacją.

**Why:** chce nowoczesnych, dopracowanych „systemów wysuwanych”; każdy element ma być sprawdzony przed wydaniem wersji.
**How to apply:** nie wracać do paska ikon ani kapsuły w belce; każda nowa zmiana paneli — test `e2e/panels-slide.spec.ts` we wszystkich stylach i motywach, zrzuty przejrzane. Tryb pracy w UI czyta poziom globalny (`app.autonomy`), nie tylko sesji. Zob. [[alfa-weryfikacja-ui]], [[alfa-pelny-dostep-ai]].
