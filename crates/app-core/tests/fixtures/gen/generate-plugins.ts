// Fixture'y komend wtyczek Wasm (`plugins_*`) — część generatora `generate.ts` (atrapa +
// `TauriAlfaClient` z atrapą `invoke`): lista, kontrola modułu, propozycja, zatwierdzenie hashem,
// odrzucenie, wyłączenie, ponowne włączenie, usunięcie.
import type { PluginInfo } from '../../../../../apps/desktop/ui/src/lib/api/types-plugins';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

/** Najmniejszy nagłówek komponentu Wasm (`\0asm`, warstwa komponentu) jako base64. */
const COMPONENT_B64 = 'AGFzbQ0AAQA=';

export async function runPlugins(both: Both): Promise<void> {
  await both('plugins', 'list');
  await both('plugins', 'inspect', COMPONENT_B64);
  await both('plugins', 'inspect', 'AGFzbQEAAAA=');
  const proposed = (await both(
    'plugins',
    'propose',
    {
      id: 'notatnik',
      version: '1.0.0',
      author: 'Właściciel',
      description: 'Dopisuje notatki do pliku w katalogu Dokumenty\\Notatki.',
      wasm_sha256: '0'.repeat(64),
      capabilities: [
        { cap: 'fs.write', scope: { path: 'C:\\Users\\Ty\\Documents\\Notatki', subtree: true } },
      ],
      tools: [
        {
          name: 'append_note',
          title: 'Dopisz notatkę',
          description: 'Dopisuje tekst na końcu pliku notatek.',
          input_schema: { type: 'object', additionalProperties: false },
          output_schema: { type: 'object' },
        },
      ],
    },
    COMPONENT_B64,
  )) as PluginInfo;
  await both('plugins', 'approve', proposed.id, proposed.version, proposed.review_hash);
  const list = (await both('plugins', 'list')) as { plugins: PluginInfo[] };
  const fx = list.plugins.find((p) => p.state === 'proposed');
  if (fx) await both('plugins', 'reject', fx.id, fx.version);
  await both('plugins', 'disable', proposed.id);
  await both('plugins', 'enable', proposed.id, proposed.review_hash);
  await both('plugins', 'remove', proposed.id);
}
