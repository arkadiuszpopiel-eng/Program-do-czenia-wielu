#!/usr/bin/env bash
# Lokalny hook pre-commit dla części UI (Svelte 5 / ui-kit). Instalacja:
#   git config core.hooksPath .githooks   # albo: cp scripts/pre-commit.sh .git/hooks/pre-commit
# Wymaga: node 22, pnpm 10, `pnpm install`.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() { printf '\n\033[1m▸ %s\033[0m\n' "$1"; }

step "prettier --check"
pnpm exec prettier --check .

step "eslint"
pnpm exec eslint .

step "reguły Svelte 5 (check-svelte5)"
node scripts/check-svelte5.mjs

step "CSS: zakaz web fontów i backdrop-filter (check-css)"
node scripts/check-css.mjs

step "tokeny designu są aktualne (build-tokens + git diff)"
pnpm --filter @alfa/ui-kit build:tokens
git diff --exit-code -- packages/ui-kit/src/tokens.css packages/ui-kit/src/tokens.ts

step "kontrast WCAG"
pnpm --filter @alfa/ui-kit check:contrast

step "svelte-check"
pnpm check

printf '\n\033[32mpre-commit: OK\033[0m\n'
