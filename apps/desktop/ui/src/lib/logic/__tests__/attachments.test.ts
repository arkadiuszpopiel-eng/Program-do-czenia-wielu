import { describe, expect, it } from 'vitest';
import type { Turn } from '../../api/types';
import type { AttachmentInfo } from '../../api/types-files';
import {
  LARGE_ATTACHMENT_TOKENS,
  carriesFiles,
  contextShare,
  estimateCost,
  hintsFrom,
  totalBytes,
  totalTokens,
} from '../attachments';

const att = (tokens: number, bytes = 100): AttachmentInfo => ({
  id: `a${tokens}`,
  session_id: 's',
  name: 'plik.txt',
  bytes,
  mime: 'text/plain',
  kind: 'text',
  path: 'C:\\in\\plik.txt',
  tokens,
  delivery: 'full',
});

const priced = (input: number, output: number, minor: number): Turn =>
  ({
    usage: {
      input_tokens: input,
      output_tokens: output,
      cost: { minor, currency: 'PLN' },
      latency_ms: 1,
      provider: 'p',
      model: 'm',
    },
  }) as Turn;

describe('załączniki: szacunki', () => {
  it('sumuje tokeny i bajty, próg dużych załączników', () => {
    const list = [att(1_000, 10), att(LARGE_ATTACHMENT_TOKENS, 20)];
    expect(totalTokens(list)).toBe(1_000 + LARGE_ATTACHMENT_TOKENS);
    expect(totalBytes(list)).toBe(30);
    expect(totalTokens([])).toBe(0);
  });

  it('koszt: lokalnie zero, w chmurze ze stawki ostatnich odpowiedzi, bez danych — brak', () => {
    expect(estimateCost(10_000, 'local', [])).toEqual({ minor: 0, basis: 'local' });
    expect(estimateCost(10_000, 'cloud', [])).toBeNull();
    expect(estimateCost(0, 'cloud', [priced(1, 1, 1)])).toBeNull();
    const turns = [priced(9_000, 1_000, 40), { usage: null } as Turn, priced(4_000, 1_000, 20)];
    // 60 gr / 15 000 tokenów → 0,004 gr/tok. × 10 000 = 40 gr.
    expect(estimateCost(10_000, 'hybrid', turns)).toEqual({ minor: 40, basis: 'rate' });
    expect(estimateCost(1, 'cloud', turns)?.minor).toBe(1);
  });

  it('udział w oknie kontekstu jest przycięty do 0–1', () => {
    expect(contextShare(50_000, 200_000)).toBe(0.25);
    expect(contextShare(500_000, 200_000)).toBe(1);
    expect(contextShare(1, 0)).toBe(0);
  });

  it('wskazówki z DataTransfer i rozpoznanie przeciągania plików', () => {
    expect(hintsFrom([{ name: 'a.png', size: 5, type: '' }])).toEqual([
      { name: 'a.png', bytes: 5, mime: 'application/octet-stream' },
    ]);
    expect(carriesFiles(['text/plain', 'Files'])).toBe(true);
    expect(carriesFiles(['text/plain'])).toBe(false);
    expect(carriesFiles(null)).toBe(false);
  });
});
