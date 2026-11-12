import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

for (const theme of ['light', 'dark'] as const) {
  test(`własna agentka: zapis → przypisanie → @adresatka → odłączenie — ${theme}`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await openApp(page, { theme });
    await waitForChat(page);
    await page.keyboard.press('Control+n');
    await page.keyboard.press('Control+,');
    await page
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Kreator agentek', exact: true })
      .click();
    await page.getByLabel('Jaka ma być nowa agentka?').fill('Agentka o imieniu Luna do pomocy.');
    await page.getByRole('button', { name: 'Zaproponuj szkic' }).click();
    await expect(page.getByLabel('Imię', { exact: true })).toHaveValue('Luna');
    await page.getByRole('button', { name: 'Pokaż podgląd' }).click();
    const preview = page.getByRole('region', { name: 'Podgląd persony' });
    await preview.getByRole('button', { name: 'Test na sucho', exact: true }).click();
    await expect(preview.getByText('Test na sucho zaliczony')).toBeVisible();
    await preview.getByRole('button', { name: 'Zapisz agentkę' }).click();
    const library = page.getByRole('region', { name: 'Agentki z Kreatora' });
    await library.getByRole('button', { name: 'Dodaj do tej rozmowy: Luna', exact: true }).click();
    await expect(
      library.getByRole('button', { name: 'Odłącz rolę od tej rozmowy: Luna' }),
    ).toBeEnabled();
    await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
    const recipient = page.getByRole('button', { name: 'Adresatka: Auto', exact: true });
    await recipient.click();
    await expect(page.getByRole('menuitemcheckbox', { name: 'luna', exact: true })).toBeVisible();
    await page.getByRole('menuitemcheckbox', { name: 'Alfa', exact: true }).click();
    // Jawne @ wygrywa nad poprzednim wyborem Alfy w menu.
    const field = page.locator('#alfa-composer');
    await field.fill('@luna Przedstaw krótki meldunek');
    await field.press('Enter');
    const answer = page.getByRole('article', { name: /Wiadomość agentki luna/ }).last();
    await expect(answer).toBeVisible();
    await expectAccessible(page, `własna adresatka ${theme}`);
    await page.getByRole('button', { name: 'Adresatka: Alfa', exact: true }).click();
    await page.getByRole('menuitemcheckbox', { name: 'luna', exact: true }).click();
    await page.keyboard.press('Control+,');
    await library.getByRole('button', { name: 'Odłącz rolę od tej rozmowy: Luna' }).click();
    await expect(library.getByRole('button', { name: 'Dodaj do tej rozmowy: Luna' })).toBeEnabled();
    await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
    await page.getByRole('button', { name: 'Adresatka: Auto', exact: true }).click();
    await expect(page.getByRole('menuitemcheckbox', { name: 'luna', exact: true })).toHaveCount(0);
    expect(errors).toEqual([]);
  });
}
