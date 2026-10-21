---
name: narzedzia-ukosniki-heredoc
description: "Na tym PC narzędzie Bash zjada podwójne ukośniki odwrotne w heredoc — kod Rust z '\\\\' i kontynuacjami wiersza edytować narzędziem Edit"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 8c1892ed-2987-4f4f-91ab-0abe9bb3c4b8
  modified: 2026-10-21T21:49:58.245Z
---

2026-10-21 w nocy (Windows, Git Bash przez narzędzie Bash) `'\\'` wpisane w heredoc z cudzysłowem
(`<<'EOF'`) trafiło do pliku jako `'\'` (błąd kompilacji Rust), a kontynuacje wiersza `\` w napisach
wstawione przez skrypt Pythona z heredoc dały ciągi spacji albo dosłowne `\n` w komunikatach.

**Why:** dwie poprawki trzeba było naprawiać drugi raz; recenzent wychwycił spacje w komunikatach
dla agentki.

**How to apply:** fragmenty z ukośnikami odwrotnymi (literały `'\\'`, `r"…\…"`, kontynuacje `\` w napisach
Rust) zmieniać narzędziem Edit, nie przez heredoc/Pythona w Bash; po wstawieniu sprawdzić `sed -n`/`cat -A`.
Zob. [[alfa-srodowisko-lokalne]].
