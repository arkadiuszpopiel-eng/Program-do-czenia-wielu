// Klient podświetlania: worker tworzony przy pierwszym zamkniętym bloku kodu. Wynik (zakresy)
// zamieniany na węzły DOM przez `textContent` — żadnego innerHTML poza SanitizedHtml.
import type { TokenRange } from './tokenize';

let worker: Worker | null = null;
let nextId = 1;
const waiting = new Map<number, (ranges: readonly TokenRange[]) => void>();

function getWorker(): Worker {
  if (!worker) {
    worker = new Worker(new URL('./highlight.worker.ts', import.meta.url), { type: 'module' });
    worker.onmessage = (event: MessageEvent<{ id: number; ranges: TokenRange[] }>) => {
      waiting.get(event.data.id)?.(event.data.ranges);
      waiting.delete(event.data.id);
    };
  }
  return worker;
}

function request(code: string, lang: string | null): Promise<readonly TokenRange[]> {
  const id = nextId++;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    getWorker().postMessage({ id, code, lang });
  });
}

/** Buduje fragment DOM z tekstu i zakresów (czysta funkcja — testowalna). */
export function buildFragment(
  doc: Document,
  code: string,
  ranges: readonly TokenRange[],
): DocumentFragment {
  const fragment = doc.createDocumentFragment();
  let at = 0;
  for (const [start, end, kind] of ranges) {
    if (start > at) fragment.append(doc.createTextNode(code.slice(at, start)));
    const span = doc.createElement('span');
    span.className = `tok-${kind}`;
    span.textContent = code.slice(start, end);
    fragment.append(span);
    at = end;
  }
  if (at < code.length) fragment.append(doc.createTextNode(code.slice(at)));
  return fragment;
}

/** Podświetla element `<code>` w miejscu (raz). */
export async function highlightElement(element: HTMLElement, lang: string | null): Promise<void> {
  if (element.dataset['highlighted'] || typeof Worker === 'undefined') return;
  element.dataset['highlighted'] = '1';
  const code = element.textContent ?? '';
  if (code.length > 200_000) return;
  const ranges = await request(code, lang);
  if (!element.isConnected || element.textContent !== code) return;
  element.replaceChildren(buildFragment(element.ownerDocument, code, ranges));
}
