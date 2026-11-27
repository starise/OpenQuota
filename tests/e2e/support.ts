import { test as base, expect, type Page } from '@playwright/test';

interface IpcCall {
  command: string;
  args: Record<string, unknown>;
}

interface E2eHarness {
  calls: IpcCall[];
  errors: string[];
  panelHeight: number;
  panelHeightMode: string;
}

declare global {
  interface Window {
    __OPENQUOTA_E2E__?: E2eHarness;
  }
}

export { expect };
export const test = base.extend({
  page: async ({ page }, use) => {
    const browserErrors: string[] = [];
    page.on('pageerror', (error) => browserErrors.push(error.stack ?? error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') browserErrors.push(message.text());
    });
    page.on('requestfailed', (request) =>
      browserErrors.push(`${request.method()} ${request.url()}: ${request.failure()?.errorText}`),
    );

    try {
      await use(page);
    } finally {
      let ipcErrors: string[] = [];
      try {
        ipcErrors = await page.evaluate(() => window.__OPENQUOTA_E2E__?.errors ?? []);
      } catch (error) {
        ipcErrors.push(`Could not read browser fixture errors: ${String(error)}`);
      }
      expect([...browserErrors, ...ipcErrors], 'browser and mocked IPC errors').toEqual([]);
    }
  },
});

export async function openApp(page: Page, height = 720) {
  await page.setViewportSize({ width: 640, height });
  await page.goto('/tests/e2e/index.html');
  await page.getByLabel('Open options').waitFor();
}
