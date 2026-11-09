// Ten sam przepływ UI dla Chromium i rzeczywistego WebView2, bez uruchamiania modelu.
import { expect, type Page } from '@playwright/test';
import { pickOption } from '../e2e/helpers';
import { prepareComposer } from './composer-ready';

async function builder(page: Page) {
  await prepareComposer(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kreator agentek', exact: true })
    .click();
  return page.getByRole('region', { name: 'Agentki z Kreatora' });
}

export async function jarvisFlow(
  page: Page,
  remember: (key: string, value: string) => void,
  snapshot: () => Promise<void> = async () => {},
) {
  let library = await builder(page);
  let common = library.getByRole('combobox', { name: 'Wspólny profil współpracy' });
  const original = await common.getAttribute('data-value');
  if (original !== 'standard' && original !== 'jarvis') throw new Error('Brak profilu wspólnego');
  remember('agents.interaction_mode', original);
  await pickOption(common, 'jarvis');
  const voice = library.getByRole('button', { name: 'Sterowanie głosem' });
  await expect(voice).toBeVisible();
  await voice.scrollIntoViewIfNeeded();
  await snapshot();
  await voice.click();
  await expect(page.getByRole('tab', { name: 'Głos', selected: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Wróć do rozmowy' })).toHaveCount(0);
  await page.keyboard.press('Alt+1');
  for (const agent of ['alfa', 'beta', 'gama', 'delta']) {
    const card = page.locator(`.interaction[data-agent="${agent}"]`);
    const select = card.getByRole('combobox');
    const before = await select.getAttribute('data-value');
    if (!before || !['inherit', 'standard', 'jarvis'].includes(before)) {
      throw new Error(`Brak profilu ${agent}`);
    }
    remember(`agents.${agent}.interaction_mode`, before);
    await pickOption(select, 'jarvis');
    await expect(card).toHaveAttribute('data-interaction', 'jarvis');
    await pickOption(select, 'standard');
    await expect(card).toHaveAttribute('data-interaction', 'standard');
    await pickOption(select, before);
  }
  library = await builder(page);
  common = library.getByRole('combobox', { name: 'Wspólny profil współpracy' });
  await expect(common).toHaveAttribute('data-value', 'jarvis');
  await pickOption(common, original);
  await prepareComposer(page);
}
