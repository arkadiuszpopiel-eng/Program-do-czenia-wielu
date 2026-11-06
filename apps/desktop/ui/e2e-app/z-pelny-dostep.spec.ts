// Instancja testowa z poziomem przygotowanym w profilu (docs/plan/45 §6.2): `ALFA_TEST_AUTONOMY=L4`
// zakłada plik zapamiętanego poziomu Brokera w profilu tymczasowym, a Broker w procesie startuje
// z tym poziomem — bez okna zgód. Bez zmiennej test jest pomijany (zwykły bieg: domyślne
// „Automatycznie”). Biegnie przed `zz-mode` (ten obniża poziom do „Ręcznie”).
import { expect, test } from './alfa';
import { call } from './ipc-call';

interface PermissionsView {
  global: string;
}

const LEVEL = process.env.ALFA_TEST_AUTONOMY ?? '';

test('instancja testowa startuje z poziomem przygotowanym w profilu', async ({ alfa }) => {
  test.skip(LEVEL === '', 'bez ALFA_TEST_AUTONOMY — poziom domyślny');
  const view = await call<PermissionsView>(alfa.main, 'permissions_get', { sessionId: null });
  expect(view.global).toBe(LEVEL);
});
