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
