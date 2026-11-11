#!/usr/bin/env node
// Odtwarzalne źródło adaptera SQLCipher: archiwum crates.io + jawna aktualizacja zależności.
// Plików vendor/libsqlite3-sys nie poprawiaj ręcznie; uruchom ten generator.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const target = join(root, 'vendor', 'libsqlite3-sys');
const archiveSha = '14a5f01b87b92705d07b433c06f96cbc764637d81a0bffb5734ce17cf0f9a2b3';
const archiveUrl = 'https://static.crates.io/crates/libsqlite3-sys/libsqlite3-sys-0.39.1.crate';
const manifestName = 'ALFA_VENDOR.json';
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
function assert(condition, message) {
  if (!condition) throw new Error(message);
}
function files(directory, prefix = '') {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      assert(!entry.isSymbolicLink(), `Niedozwolone dowiązanie: ${entry.name}`);
      const name = prefix + entry.name;
      return entry.isDirectory() ? files(join(directory, entry.name), name + '/') : [name];
    })
    .sort();
}
function verify() {
  const manifest = JSON.parse(readFileSync(join(target, manifestName), 'utf8'));
  assert(manifest.archive_sha256 === archiveSha, 'Niezgodny pin źródła vendora');
  const actual = files(target).filter((p) => p !== manifestName);
  assert(
    JSON.stringify(actual) === JSON.stringify(Object.keys(manifest.files).sort()),
    'Niezgodny zbiór plików vendora',
  );
  for (const name of actual) {
    assert(
      sha(readFileSync(join(target, name))) === manifest.files[name],
      `Zmieniony vendor: ${name}`,
    );
  }
  assert(
    readFileSync(join(target, 'Cargo.toml'), 'utf8').includes('version = "=420.0.0"'),
    'Brak poprawionego SQLCipher',
  );
  console.log(`vendor-sqlite: ${actual.length} plików; odciski i pin zgodne`);
}
function generate(archive) {
  assert(sha(readFileSync(archive)) === archiveSha, 'Niezgodny SHA256 archiwum crates.io');
  const entries = execFileSync('tar', ['-tf', archive], { encoding: 'utf8' }).trim().split(/\r?\n/);
  assert(
    entries.every(
      (p) =>
        p.startsWith('libsqlite3-sys-0.39.1/') && !p.split('/').includes('..') && !p.includes('\\'),
    ),
    'Niedozwolona ścieżka archiwum',
  );
  const temp = mkdtempSync(join(tmpdir(), 'alfa-sqlite-vendor-'));
  try {
    execFileSync('tar', ['-xf', archive, '-C', temp, '--strip-components=1']);
    // Wspieramy istniejącą konfigurację szyfrowaną; pomijamy nieużywany zwykły silnik.
    const omitted = [
      'sqlite3/sqlite3.c',
      'sqlite3/sqlite3.h',
      'sqlite3/sqlite3ext.h',
      'Cargo.lock',
    ];
    for (const name of omitted) rmSync(join(temp, name));
    // Zmiana rzeczywiście budowanego SQLCipher; sama wersja adaptera nie usuwa CVE.
    for (const name of ['Cargo.toml', 'Cargo.toml.orig']) {
      const path = join(temp, name);
      const source = readFileSync(path, 'utf8');
      assert(source.split('version = "=419.0.6"').length === 2, `Nieoczekiwany manifest: ${name}`);
      writeFileSync(path, source.replace('version = "=419.0.6"', 'version = "=420.0.0"'));
    }
    // Zachowaj logikę istniejących modułów; osobne pliki mieszczą się w limicie 400 linii.
    let build = readFileSync(join(temp, 'build.rs'), 'utf8');
    const modules = [
      ...build.matchAll(/^mod (build_bundled|build_linked|bindings|loadable_extension) \{\n/gm),
    ];
    assert(modules.length === 5, 'Zmieniła się struktura upstream build.rs');
    mkdirSync(join(temp, 'build'));
    let bindings = 0;
    const extracted = modules.map((match) => {
      const end = build.indexOf('\n}\n', match.index) + 1;
      assert(end > match.index, 'Nie znaleziono końca modułu');
      const stem =
        match[1] === 'bindings'
          ? ['bindings_prebuilt', 'bindings_buildtime'][bindings++]
          : match[1];
      const body = build.slice(match.index + match[0].length, end);
      assert(body.split('\n').length <= 400, `Za długi moduł: ${stem}`);
      writeFileSync(
        join(temp, 'build', stem + '.rs'),
        '// Wygenerowano z przypiętego źródła; logika upstream bez zmian.\n' + body,
      );
      return {
        start: match.index,
        end: end + 2,
        text: `#[path = "build/${stem}.rs"]\nmod ${match[1]};\n`,
      };
    });
    for (const item of extracted.reverse())
      build = build.slice(0, item.start) + item.text + build.slice(item.end);
    writeFileSync(join(temp, 'build.rs'), build);
    // Wiązania bindgen bez zmiany treści; poniżej jawna normalizacja pustych końców plików.
    const generated = [];
    for (const name of files(temp).filter((p) => /(^|\/)bindgen[^/]*\.rs$/.test(p))) {
      const bytes = readFileSync(join(temp, name));
      const moved = join(dirname(name), 'generated', name.split('/').at(-1)).replaceAll('\\', '/');
      mkdirSync(dirname(join(temp, moved)), { recursive: true });
      writeFileSync(join(temp, moved), bytes);
      writeFileSync(
        join(temp, name),
        `// Wiązania bindgen w katalogu generated; nie edytuj ręcznie.\ninclude!(concat!(env!("CARGO_MANIFEST_DIR"), "/${moved}"));\n`,
      );
      generated.push({ original: name, generated: moved, sha256: sha(bytes) });
    }
    assert(generated.length === 5, 'Zmienił się zbiór wiązań upstream');
    writeFileSync(
      join(temp, 'README.alfa.md'),
      `# Adapter SQLCipher Alfy\n\nŹródło: ${archiveUrl}\n\nSHA256: ${archiveSha}.\n\nZmiany: SQLCipher-src420.0.0, rozdzielenie istniejących modułów build.rs,\nprzeniesienie wiązań bindgen bez zmian tokenów; jawna normalizacja białych znaków w czterech plikach. Obsługiwany jest szyfrowany wariant bundled-sqlcipher; nie dostarczamy nieużywanego\nzwykłego silnika SQLite ani obcego Cargo.lock. Bez zmiany polityki źródeł.\n\nOdtworzenie: node scripts/vendor-sqlite.mjs /ścieżka/do/libsqlite3-sys-0.39.1.crate.\nSprawdzenie offline: node scripts/vendor-sqlite.mjs --check.\n`,
    );
    // Format własnych adapterów i wyodrębnionych modułów; wiązania bindgen pozostają nietknięte.
    const rustFiles = files(temp).filter((p) => p.endsWith('.rs') && !p.includes('/generated/'));
    execFileSync(process.env.RUSTFMT || 'rustfmt', [
      '--edition',
      '2024',
      '--config',
      'skip_children=true',
      ...rustFiles.map((p) => join(temp, p)),
    ]);
    const whitespace = [];
    for (const name of [
      'sqlite3/wasm32-wasi-vfs.c',
      'wrapper_ext.h',
      'bindgen-bindings/generated/bindgen_3.45.3_ext.rs',
      'sqlite3/generated/bindgen_bundled_version_ext.rs',
    ]) {
      const before = readFileSync(join(temp, name));
      const after = Buffer.from(
        before
          .toString('utf8')
          .replace(/[ \t]+$/gm, '')
          .replace(/\n+$/, '\n'),
      );
      whitespace.push({ path: name, upstream_sha256: sha(before), normalized_sha256: sha(after) });
      writeFileSync(join(temp, name), after);
    }
    for (const item of generated) {
      item.upstream_sha256 = item.sha256;
      item.sha256 = sha(readFileSync(join(temp, item.generated)));
    }
    const hashes = Object.fromEntries(
      files(temp).map((p) => [p, sha(readFileSync(join(temp, p)))]),
    );
    writeFileSync(
      join(temp, manifestName),
      JSON.stringify(
        {
          archive_url: archiveUrl,
          archive_sha256: archiveSha,
          adapter: '0.39.1',
          sqlcipher_src: '=420.0.0',
          omitted_upstream_files: omitted,
          generated_bindings: generated,
          whitespace_normalizations: whitespace,
          files: hashes,
        },
        null,
        2,
      ) + '\n',
    );
    mkdirSync(dirname(target), { recursive: true });
    if (existsSync(target)) rmSync(target, { recursive: true });
    cpSync(temp, target, { recursive: true });
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
  verify();
}
if (process.argv[2] === '--check') verify();
else {
  assert(process.argv.length === 3, 'Podaj archiwum libsqlite3-sys0.39.1.crate albo --check');
  generate(resolve(process.argv[2]));
}
