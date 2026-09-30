// Web Worker podświetlania składni (PLAN §14.7): ładowany leniwie, poza wątkiem głównym.
import { tokenize } from './tokenize';

interface Request {
  readonly id: number;
  readonly code: string;
  readonly lang: string | null;
}

self.onmessage = (event: MessageEvent<Request>) => {
  const { id, code, lang } = event.data;
  self.postMessage({ id, ranges: tokenize(code, lang) });
};
