// Trusted Types (PLAN §8.2, THREAT_MODEL S12, PT-33): CSP wydania ma
// `require-trusted-types-for 'script'`, więc surowe przypisania HTML, adresów skryptów i `eval`
// rzucają wyjątek. Dozwolone polityki (`trusted-types` w tauri.conf.json):
// - `svelte-trusted-html` — szablony Svelte (tworzy ją środowisko Svelte),
// - `alfa-sanitized-html` — wyłącznie `SanitizedHtml` (HTML zsanitowany w Rust: pulldown-cmark
//   + ammonia, ADR 0009),
// - `default` — tylko adresy skryptów z tego samego pochodzenia (worker podświetlania); bez
//   `createHTML` i `createScript`, więc każde inne `innerHTML`/`eval` w UI zostaje zablokowane.
// Bez Trusted Types w środowisku (testy jednostkowe, inne przeglądarki) — zwykłe łańcuchy.

interface TrustedPolicy {
  createHTML?: (input: string) => unknown;
  createScriptURL?: (input: string) => unknown;
}

interface TrustedTypesFactory {
  createPolicy(
    name: string,
    rules: { createHTML?: (input: string) => string; createScriptURL?: (input: string) => string },
  ): TrustedPolicy;
}

function factory(): TrustedTypesFactory | undefined {
  return (globalThis as { trustedTypes?: TrustedTypesFactory }).trustedTypes;
}

function create(
  name: string,
  rules: { createHTML?: (input: string) => string; createScriptURL?: (input: string) => string },
): TrustedPolicy | undefined {
  try {
    return factory()?.createPolicy(name, rules);
  } catch (error) {
    // Nazwa spoza `trusted-types` albo powtórzona — sink i tak zostanie zablokowany przez CSP.
    console.error(`Trusted Types: polityka ${name} niedostępna`, error);
    return undefined;
  }
}

/** Nazwa polityki HTML z backendu (musi być w `trusted-types` CSP). */
export const SANITIZED_HTML_POLICY = 'alfa-sanitized-html';

// HTML przychodzi już zsanitowany z rdzenia — polityka go nie zmienia, tylko oznacza jako zaufany.
const sanitizedPolicy = create(SANITIZED_HTML_POLICY, { createHTML: (html) => html });

/**
 * HTML z `RenderedBlock.html_sanitized` jako wartość dla `{@html}` (pod CSP — `TrustedHTML`,
 * który Svelte przyjmuje; typ `string` tylko dla kompilatora). Nie używaj dla innych treści.
 */
export function sanitizedHtml(html: string): string {
  return (sanitizedPolicy?.createHTML?.(html) ?? html) as string;
}

/** Adres skryptu dozwolony przez politykę `default`: wyłącznie to samo pochodzenie co strona. */
export function sameOriginScriptUrl(url: string, page: string): string {
  const parsed = new URL(url, page);
  if (parsed.origin !== new URL(page).origin) {
    throw new TypeError(`Trusted Types: skrypt spoza aplikacji (${parsed.origin})`);
  }
  return url;
}

let installed = false;

/** Polityka `default` (raz na stronę, przed pierwszym workerem): tylko adresy skryptów. */
export function installDefaultPolicy(): void {
  if (installed) return;
  installed = true;
  create('default', {
    createScriptURL: (url) => sameOriginScriptUrl(url, globalThis.location.href),
  });
}
