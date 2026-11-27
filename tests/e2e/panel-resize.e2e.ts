import { expect, openApp, test } from './support';

test('resizes the panel from the keyboard and returns to automatic height', async ({ page }) => {
  await openApp(page, 400);
  const grip = page.getByRole('separator', { name: 'Resize panel height' });
  await expect(grip).toBeVisible();

  for (let tab = 0; tab < 60; tab += 1) {
    await page.keyboard.press('Tab');
    if (await grip.evaluate((element) => element === document.activeElement)) break;
  }

  await expect(grip).toBeFocused();
  await expect(grip).toHaveCSS('outline-style', 'solid');
  await expect(grip).toHaveCSS('outline-width', '2px');
  await expect(grip).toHaveAttribute('aria-valuetext', '400 pixels');

  await page.keyboard.press('ArrowDown');
  await expect(grip).toHaveAttribute('aria-valuetext', '424 pixels');
  await expect
    .poll(() => page.evaluate(() => window.__OPENQUOTA_E2E__?.panelHeightMode))
    .toBe('manual');
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__OPENQUOTA_E2E__?.calls.filter(
          ({ command }) => command === 'set_panel_height_manual',
        ),
      ),
    )
    .toEqual([{ command: 'set_panel_height_manual', args: { adjustment: 24 } }]);

  await page.keyboard.press('Enter');
  await expect
    .poll(() => page.evaluate(() => window.__OPENQUOTA_E2E__?.panelHeightMode))
    .toBe('automatic');
  await expect(grip).toBeFocused();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__OPENQUOTA_E2E__?.calls.some(
          ({ command }) => command === 'set_panel_height_automatic',
        ),
      ),
    )
    .toBe(true);
});
