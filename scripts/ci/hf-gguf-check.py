"""Sprawdza modele GGUF z `models.toml` w API Hugging Face — bez pobierania plików.

Dla każdego `[[model]]`: plik musi istnieć w repozytorium (rewizja z adresu `resolve/<rewizja>/`),
rozmiar z API musi zgadzać się z `size_mb` (±1 MiB), a przypięty `sha256` (jeśli jest) — z hashem
LFS. Wynik: tabela w `$GITHUB_STEP_SUMMARY` (gdy ustawione) i na wyjściu; kod 1 przy niezgodności.
Tabela SHA-256 służy człowiekowi do przypięcia hashy w katalogu (skrypt niczego nie przypina).

Użycie: python3 scripts/ci/hf-gguf-check.py crates/providers-local-impl/models.toml
`HF_ENDPOINT` (domyślnie https://huggingface.co) — podmiana adresu API w testach.
"""

import json
import os
import re
import sys
import tomllib
import urllib.error
import urllib.parse
import urllib.request

MIB = 1 << 20
URL = re.compile(r"^https://huggingface\.co/([^/]+/[^/]+)/resolve/([^/]+)/(.+)$")


def emit(lines):
    text = "\n".join(lines)
    print(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as out:
            out.write(text + "\n")


def repo_files(endpoint, repo, revision):
    url = f"{endpoint}/api/models/{repo}/revision/{urllib.parse.quote(revision, safe='')}?blobs=true"
    with urllib.request.urlopen(url, timeout=30) as response:
        info = json.load(response)
    return {s["rfilename"]: s for s in info.get("siblings", [])}


def check(model, endpoint, cache):
    """Zwraca (wiersz tabeli, czy poprawny)."""
    found = URL.match(model["url"])
    if not found:
        return f"| `{model['id']}` | — | — | ❌ adres spoza `huggingface.co/<repo>/resolve/` |", False
    repo, revision, path = found.groups()
    key = (repo, revision)
    if key not in cache:
        try:
            cache[key] = repo_files(endpoint, repo, revision)
        except (urllib.error.URLError, OSError, ValueError) as err:
            cache[key] = err
    files = cache[key]
    if isinstance(files, Exception):
        return f"| `{model['id']}` | — | — | ❌ API: {files} |", False
    entry = files.get(urllib.parse.unquote(path))
    if entry is None:
        return f"| `{model['id']}` | {model['size_mb']} / brak | — | ❌ brak pliku `{path}` w `{repo}@{revision}` |", False
    lfs = entry.get("lfs") or {}
    size = lfs.get("size") or entry.get("size") or 0
    sha = lfs.get("sha256", "")
    problems = []
    if abs(size / MIB - model["size_mb"]) > 1:
        problems.append(f"rozmiar {size / MIB:.1f} MiB ≠ size_mb {model['size_mb']}")
    pinned = model.get("sha256", "")
    if pinned and pinned.lower() != sha.lower():
        problems.append("przypięty sha256 ≠ hash LFS")
    if not sha:
        problems.append("brak hasha LFS")
    verdict = "❌ " + "; ".join(problems) if problems else ("✅ przypięty" if pinned else "✅ (do przypięcia)")
    return f"| `{model['id']}` | {model['size_mb']} / {size // MIB} | `{sha or '?'}` | {verdict} |", not problems


def main(argv):
    if len(argv) != 2:
        print(__doc__)
        return 2
    with open(argv[1], "rb") as manifest:
        models = tomllib.load(manifest).get("model", [])
    endpoint = os.environ.get("HF_ENDPOINT", "https://huggingface.co").rstrip("/")
    cache = {}
    rows = [check(model, endpoint, cache) for model in models]
    emit(
        [
            f"## Modele GGUF z `{argv[1]}` (API Hugging Face, bez pobierania)",
            "",
            "| id | MiB katalog / HF | SHA-256 (LFS) | wynik |",
            "|---|---|---|---|",
            *(row for row, _ in rows),
        ]
    )
    if not models:
        print("brak wpisów [[model]]", file=sys.stderr)
        return 1
    return 0 if all(ok for _, ok in rows) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
