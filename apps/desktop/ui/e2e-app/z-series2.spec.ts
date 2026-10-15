// Seria 2 (0.0.61) na PRAWDZIWYM rdzeniu: instrukcja wbudowana (spis, dział, wyszukiwanie),
// Tablica (lista, odmowa przypięcia nie-agentki, odpięcie nieznanego) i trasy zgodności.
import { expect, test } from './alfa';
import { call, refused } from './ipc-call';

interface HelpTopic {
  id: string;
  title: string;
}

test('instrukcja i Tablica na prawdziwym rdzeniu', async ({ alfa }) => {
  const { main } = alfa;
  const topics = await call<HelpTopic[]>(main, 'help_topics');
  expect(topics.length).toBeGreaterThanOrEqual(10);
  const page = await call<{ html_sanitized: string }>(main, 'help_page', { id: 'rozmowa' });
  expect(page.html_sanitized).toContain('<h1');
  expect(page.html_sanitized).not.toContain('<script');
  await refused(main, 'help_page', { id: 'nie-ma-takiego' });
  const hits = await call<{ id: string }[]>(main, 'help_search', {
    query: 'polecenia bez modelu',
  });
  expect(hits.some((h) => h.id === 'rozmowa')).toBe(true);

  const board = await call<unknown[]>(main, 'board_list');
  expect(Array.isArray(board)).toBe(true);
  await refused(main, 'board_pin', { sessionId: 's-nie-ma', turnId: 't1' });
  expect(await call<unknown[]>(main, 'board_unpin', { id: 'pin-nie-ma' })).toEqual(board);
});
