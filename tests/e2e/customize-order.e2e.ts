import { expect, openApp, test } from './support';

test('reorders providers in Customize with a mouse drag', async ({ page }) => {
  await openApp(page);
  await page.getByLabel('Open options').click();
  await page.getByRole('button', { name: 'Customize' }).click();

  const list = page.getByRole('region', { name: 'Customize' }).getByRole('list');
  const rows = list.getByRole('listitem');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText('Codex');
  await expect(rows.nth(1)).toContainText('Claude');

  const handle = page.getByRole('button', { name: 'Move Codex' });
  const target = await rows.nth(1).boundingBox();
  const start = await handle.boundingBox();
  if (!target || !start) throw new Error('Provider reorder rows are not visible.');

  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await page.mouse.down();
  await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2, { steps: 6 });
  await page.mouse.up();

  await expect(rows.nth(0)).toContainText('Claude');
  await expect(rows.nth(1)).toContainText('Codex');
  await expect(handle).toHaveAttribute('aria-expanded', 'false');
  await expect
    .poll(() =>
      page.evaluate(() => {
        const saves = window.__OPENQUOTA_E2E__?.calls.filter(
          ({ command }) => command === 'save_app_settings',
        );
        const settings = saves?.at(-1)?.args.settings as
          | { providers?: { id: string }[] }
          | undefined;
        return settings?.providers?.map(({ id }) => id);
      }),
    )
    .toEqual(['claude', 'codex']);
});

test('moves a provider from its grip controls and returns focus to that provider', async ({
  page,
}) => {
  await openApp(page);
  await page.getByLabel('Open options').click();
  await page.getByRole('button', { name: 'Customize' }).click();

  const customize = page.getByRole('region', { name: 'Customize' });
  const rows = customize.getByRole('listitem');
  const grip = customize.getByRole('button', { name: 'Move Codex', exact: true });
  const moveUp = customize.getByRole('button', { name: 'Move up', exact: true });
  const moveDown = customize.getByRole('button', { name: 'Move down', exact: true });

  await grip.focus();
  await page.keyboard.press('Space');
  await expect(grip).toHaveAttribute('aria-expanded', 'true');
  await expect(moveUp).toBeDisabled();
  await expect(moveDown).toBeFocused();
  const gripBounds = await grip.boundingBox();
  const moveBounds = await moveDown.boundingBox();
  if (!gripBounds || !moveBounds) throw new Error('Provider reorder controls are not visible.');
  expect(moveBounds.y).toBeGreaterThanOrEqual(gripBounds.y + gripBounds.height);
  await page.keyboard.press('Enter');

  await expect(rows.nth(0)).toContainText('Claude');
  await expect(rows.nth(1)).toContainText('Codex');
  await expect(grip).toBeFocused();
  await expect(grip).toHaveAttribute('aria-expanded', 'false');

  await grip.click();
  await expect(moveUp).toBeFocused();
  await expect(moveDown).toBeDisabled();
  await page.keyboard.press('Escape');
  await expect(grip).toBeFocused();
  await expect(moveUp).toBeHidden();
  await expect(customize).toBeVisible();

  await grip.click();
  await moveUp.click();
  await expect(rows.nth(0)).toContainText('Codex');
  await expect(grip).toBeFocused();
});
