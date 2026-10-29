import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Zlecenie właściciela (2026-10-09): system kamer — kilka kamer naraz, podgląd, przełączniki,
// widoczny znak działającej kamery. Tu strona „Kamery” i wskaźnik w belce na atrapie.

const SKINS = ['classic', 'glass', 'next', 'junior'] as const;
const THEMES = ['light', 'dark'] as const;

async function openCameras(page: Page, query = '') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto(`/${query}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kamery', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Twoje kamery' })).toBeVisible();
}

const card = (page: Page, id: string) => page.locator(`li.camera[data-camera="${id}"]`);
const indicator = (page: Page) => page.locator('header.titlebar button.cam');
const master = (page: Page) => page.getByRole('switch', { name: 'Kamery włączone' });

test('kamery są domyślnie wyłączone: nic się nie otwiera, podgląd jest niedostępny', async ({
  page,
}) => {
  await openCameras(page);
  await expect(master(page)).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByText('Kamery są wyłączone — żadna się nie otworzy.')).toBeVisible();
  await expect(page.locator('li.camera')).toHaveCount(3);
  for (const button of await page.getByRole('button', { name: 'Podgląd', exact: true }).all()) {
    await expect(button).toBeDisabled();
  }
  await expect(indicator(page)).toHaveCount(0);
  await expect(page.locator('li.camera img')).toHaveCount(0);
});

test('dwie kamery naraz: podgląd na żywo, wskaźnik w belce, uśpienie po zakończeniu', async ({
  page,
}) => {
  await openCameras(page);
  await master(page).click();
  await expect(page.getByText('Kamery są wyłączone')).toHaveCount(0);
  const first = card(page, 'usb-integrated');
  const second = card(page, 'usb-logi-c920');
  await first.getByRole('button', { name: 'Podgląd', exact: true }).click();
  await second.getByRole('button', { name: 'Podgląd', exact: true }).click();

  // Obie kamery działają jednocześnie, a obraz się zmienia (kolejne klatki).
  await expect(first.locator('img')).toBeVisible();
  await expect(second.locator('img')).toBeVisible();
  await expect(first.locator('.state')).toHaveText('działa');
  await expect(second.locator('.state')).toHaveText('działa');
  const before = await first.locator('img').getAttribute('src');
  await expect
    .poll(async () => first.locator('img').getAttribute('src'), { timeout: 5_000 })
    .not.toBe(before);

  // Działające kamery zawsze widać w belce; klik prowadzi do ich ustawień.
  await expect(indicator(page)).toBeVisible();
  await expect(indicator(page)).toHaveAccessibleName(/^Działają 2 kamery: /);
  await expect(indicator(page).locator('.count')).toHaveText('2');

  // Koniec podglądu jednej kamery: druga działa dalej; pierwsza po chwili zasypia.
  await first.getByRole('button', { name: 'Zakończ podgląd' }).click();
  await expect(first.locator('img')).toHaveCount(0);
  await expect(second.locator('img')).toBeVisible();
  await expect(first.locator('.state')).toHaveText('uśpiona', { timeout: 8_000 });
  await expect(indicator(page)).toHaveAccessibleName(/^Działa 1 kamera: Logitech C920/);

  // Przełącznik główny zamyka wszystko od razu.
  await master(page).click();
  await expect(page.locator('li.camera img')).toHaveCount(0);
  await expect(indicator(page)).toHaveCount(0);
});

test('wyłączenie jednej kamery, kamera odłączona, zmiana nazwy', async ({ page }) => {
  await openCameras(page);
  await master(page).click();
  const first = card(page, 'usb-integrated');
  await first.getByRole('button', { name: 'Podgląd', exact: true }).click();
  await expect(first.locator('img')).toBeVisible();
  // Wyłączenie kamery kończy jej podgląd i blokuje przycisk.
  await first.getByRole('switch', { name: 'Kamera Integrated Camera włączona' }).click();
  await expect(first.locator('img')).toHaveCount(0);
  await expect(first.getByRole('button', { name: 'Podgląd', exact: true })).toBeDisabled();
  await expect(first.getByText('wyłączona', { exact: true })).toBeVisible();

  // Odłączona kamera jest na liście, ale nie da się jej oglądać ani włączyć.
  const old = card(page, 'usb-old');
  await expect(old.locator('.state')).toHaveText('odłączona');
  await expect(old.getByRole('button', { name: 'Podgląd', exact: true })).toBeDisabled();
  await expect(old.getByRole('switch')).toBeDisabled();

  // Nazwa: Escape anuluje i nie zamyka Ustawień; Enter zapisuje.
  await first.getByRole('button', { name: 'Zmień nazwę kamery Integrated Camera' }).click();
  await page.getByLabel('Nazwa kamery').fill('Coś innego');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'Twoje kamery' })).toBeVisible();
  await expect(first.locator('.name')).toHaveText('Integrated Camera');
  await first.getByRole('button', { name: 'Zmień nazwę kamery Integrated Camera' }).click();
  await page.getByLabel('Nazwa kamery').fill('Biurko');
  await page.keyboard.press('Enter');
  await expect(first.locator('.name')).toHaveText('Biurko');
  await page.getByRole('button', { name: 'Szukaj kamer' }).click();
  await expect(first.locator('.name')).toHaveText('Biurko');
});

for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`kamery: czytelność i dostępność z podglądem — ${skin}/${theme}`, async ({ page }) => {
      await openCameras(page, `?style=${skin}&theme=${theme}`);
      await expectReadable(page, 'section.content');
      await master(page).click();
      await card(page, 'usb-integrated')
        .getByRole('button', { name: 'Podgląd', exact: true })
        .click();
      await expect(card(page, 'usb-integrated').locator('img')).toBeVisible();
      await expect(indicator(page)).toBeVisible();
      await expectReadable(page, 'section.content');
      await expectReadable(page, 'header.titlebar');
      await expectAccessible(page, `kamery ${skin} ${theme}`);
    });
  }
}

test('zdjęcie z kamery jako załącznik: przycisk tylko przy włączonych kamerach, wybór kamery', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  // Kamery wyłączone — w polu pisania nie ma przycisku zdjęcia.
  await expect(page.getByRole('button', { name: /^Dołącz zdjęcie z kamery/ })).toHaveCount(0);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kamery', exact: true })
    .click();
  await master(page).click();
  await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();

  // Dwie kamery do wyboru → menu; wybrana kamera dodaje załącznik PNG do wiadomości.
  await page.getByRole('button', { name: 'Dołącz zdjęcie z kamery', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Logitech C920' }).click();
  await expect(page.getByText(/^kamera-usb-logi-c920-\d+\.png/)).toBeVisible();
});

test('„opisz mi to, co widzisz” samo dołącza zdjęcie z kamery; bez kamer i po wyłączeniu pomocy — nie', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  const sent = page.locator('.turn, article').filter({ hasText: /opisz mi to, co widzisz/i });

  // Kamery wyłączone: zwykła wiadomość, bez zdjęcia.
  await field.fill('Opisz mi to, co widzisz');
  await field.press('Enter');
  await expect(sent.first()).toBeVisible();
  await expect(page.getByText(/kamera-usb-[a-z0-9-]+\.png/)).toHaveCount(0);

  // Kamery włączone: prośba z nazwą kamery dołącza zdjęcie z tej kamery.
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kamery', exact: true })
    .click();
  await master(page).click();
  await expect(page.getByRole('switch', { name: /Opisz, co widzisz/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
  await field.fill('Co widzisz na kamerze Logitech C920?');
  await field.press('Enter');
  // Pierwszy Enter tylko dołącza zdjęcie: widać je przy polu, tekst zostaje, nic nie poszło.
  const chip = page.getByText(/kamera-usb-logi-c920-\d+\.png/);
  await expect(chip.first()).toBeVisible();
  await expect(field).toHaveValue('Co widzisz na kamerze Logitech C920?');
  await expect(page.getByText(/Dołączyłam zdjęcie z kamery „Logitech C920”/)).toBeVisible();
  const asked = page
    .locator('.turn, article')
    .filter({ hasText: 'Co widzisz na kamerze Logitech' });
  await expect(asked).toHaveCount(0);
  // Drugi Enter wysyła wiadomość ze zdjęciem.
  await field.press('Enter');
  await expect(asked.first()).toBeVisible();
  await expect(field).toHaveValue('');

  // Pytanie o kod z „co widzisz” nie robi zdjęcia.
  await field.fill('Co widzisz w tym kodzie?');
  await field.press('Enter');
  await expect(field).toHaveValue('');
  await expect(page.getByText(/kamera-usb-[a-z0-9-]+\.png/)).toHaveCount(1);

  // Zwykłe zdanie z „widzisz” niczego nie dołącza.
  await field.fill('Jak widzisz, raport jest gotowy.');
  await field.press('Enter');
  await expect(page.getByText(/kamera-usb-[a-z0-9-]+\.png/)).toHaveCount(1);
});
