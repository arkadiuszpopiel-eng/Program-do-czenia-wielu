// Dokowanie wymaga szerokiego viewportu; runner Windows może mieć pulpit1024px.
import { expect, type Page } from '@playwright/test';

const metrics = (page: Page) =>
  page.evaluate(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
    dpr: window.devicePixelRatio,
  }));

/** Emulowany viewport tylko na fragment testu dokowania; potem wraca poprzedni lub natywny. */
export async function withDockingViewport(page: Page, run: () => Promise<void>): Promise<void> {
  const before = await metrics(page);
  const hadViewport = page.viewportSize() !== null;
  const cdp = await page.context().newCDPSession(page);
  try {
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1600,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await expect.poll(() => metrics(page)).toEqual({ width: 1600, height: 900, dpr: 1 });
    await run();
  } finally {
    try {
      if (hadViewport) {
        await cdp.send('Emulation.setDeviceMetricsOverride', {
          width: before.width,
          height: before.height,
          deviceScaleFactor: before.dpr,
          mobile: false,
        });
      } else {
        await cdp.send('Emulation.clearDeviceMetricsOverride');
      }
      await expect.poll(() => metrics(page)).toEqual(before);
    } finally {
      await cdp.detach();
    }
  }
}
