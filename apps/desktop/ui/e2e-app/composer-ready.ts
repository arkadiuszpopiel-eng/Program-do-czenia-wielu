// Wspólny prewarunek testów formularza w prawdziwej aplikacji i Chromium.
import { expect, type Page } from '@playwright/test';

export async function prepareComposer(page: Page) {
  const back = page.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  // Formularz za modalem jest „widoczny”, lecz jego przyciski nie otrzymują kliknięć.
  const drawer = page.getByRole('dialog', { name: 'Panel boczny', exact: true });
  if (await drawer.isVisible()) {
    await drawer.getByRole('button', { name: /^Schowaj panel prawy/ }).click();
    await expect(drawer).toHaveCount(0);
  }
  const composer = page.locator('form.composer');
  await expect(composer).toBeVisible({ timeout: 30_000 });
  return composer;
}
