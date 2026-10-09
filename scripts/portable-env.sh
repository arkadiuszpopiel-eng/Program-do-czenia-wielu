#!/usr/bin/env bash
# Alfa: środowisko z dysku przenośnego dla Git Bash (odpowiednik portable-env.ps1).
# Użycie: `source scripts/portable-env.sh`. Narzędzia leżą obok repozytorium w `../alfa-narzedzia`;
# litera dysku nie ma znaczenia. Narzędzie, którego na dysku nie ma, zostaje systemowe.

_alfa_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
_alfa_tools="$(cd "$_alfa_root/.." && pwd)/alfa-narzedzia"

if [ -d "$_alfa_tools/rustup/toolchains" ] && [ -x "$_alfa_tools/cargo/bin/cargo.exe" ]; then
  RUSTUP_HOME="$(cygpath -w "$_alfa_tools/rustup")"
  CARGO_HOME="$(cygpath -w "$_alfa_tools/cargo")"
  export RUSTUP_HOME CARGO_HOME
  PATH="$_alfa_tools/cargo/bin:$PATH"
fi

# Strawberry Perl przed Perlem z Gita — skrypty OpenSSL (SQLCipher) wymagają Perla dla MSWin32.
if [ -x "$_alfa_tools/StrawberryPerl/perl/bin/perl.exe" ]; then
  PATH="$_alfa_tools/StrawberryPerl/perl/bin:$_alfa_tools/StrawberryPerl/c/bin:$PATH"
elif [ -x /c/Strawberry/perl/bin/perl.exe ]; then
  PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
fi

if [ -x "$_alfa_tools/nodejs/node.exe" ]; then
  PATH="$_alfa_tools/nodejs:$PATH"
fi

# Bez `cygpath` (np. WSL) zmienne byłyby puste i npm pisałby do katalogu bieżącego.
if [ -d "$_alfa_tools" ] && command -v cygpath >/dev/null 2>&1; then
  npm_config_cache="$(cygpath -w "$_alfa_tools/npm-cache")"
  npm_config_store_dir="$(cygpath -w "$_alfa_tools/pnpm-store")"
  export npm_config_cache npm_config_store_dir
fi

export PATH
unset _alfa_root _alfa_tools
