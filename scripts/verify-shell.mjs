// Powłoka i otoczenie etapów `scripts/verify.mjs` na Windows (wydzielone: limit 400 linii na plik).
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

/**
 * Powłoka etapów. Na Windows samo `bash` trafiało na alias WSL ze Sklepu
 * (`%LOCALAPPDATA%\Microsoft\WindowsApps\bash.exe`) — bez dystrybucji WSL każdy etap kończył się w 0 s
 * (2026-10-17, komputer stacjonarny). Bierzemy Git Bash: `ALFA_BASH` z otoczenia albo `usr\bin\bash.exe`
 * z instalacji Git for Windows — ten nie przestawia PATH. `bin\bash.exe` dokleja na początek `/usr/bin`
 * i budowa OpenSSL brała okrojony perl z Git zamiast Strawberry Perl z `alfa-narzedzia` (`portable-env`).
 */
export function gitBash() {
  if (process.env.ALFA_BASH) return process.env.ALFA_BASH;
  if (process.platform !== 'win32') return 'bash';
  const roots = [process.env.ProgramFiles, 'C:\\Program Files'].filter(Boolean);
  const found = roots
    .flatMap((r) => [join(r, 'Git', 'usr', 'bin', 'bash.exe'), join(r, 'Git', 'bin', 'bash.exe')])
    .find((p) => existsSync(p));
  return found ?? 'bash';
}

/**
 * Otoczenie etapów (Windows): PATH = przenośne narzędzia z `alfa-narzedzia` i każdy Strawberry Perl
 * (przenośny albo systemowy) → katalog Git Bash (`bash`, `xargs`, `sed`…) → reszta PATH w dawnej
 * kolejności. Perl i cargo z narzędzi wygrywają z perlem z Git, a `bash` z Git — z aliasem WSL
 * (`#!/usr/bin/env bash` w `npx`), gdziekolwiek ten alias stoi w PATH. Na Windows klucz bywa `Path`.
 */
export function stageEnv(BASH, extra) {
  const env = { ...process.env, ...extra };
  if (process.platform === 'win32' && BASH !== 'bash') {
    const key = Object.keys(env).find((k) => k.toUpperCase() === 'PATH') ?? 'PATH';
    const parts = (env[key] ?? '').split(';').filter(Boolean);
    const first = (p) => /alfa-narzedzia|strawberry/i.test(p);
    env[key] = [...parts.filter(first), dirname(BASH), ...parts.filter((p) => !first(p))].join(';');
  }
  return env;
}
