import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { liveState, providerCatalog, settingsState } from '../../src/test/appFixtures';

let panelHeight = window.innerHeight;
let panelHeightMode = 'automatic';
let currentSettings = structuredClone(settingsState);
const calls: { command: string; args: Record<string, unknown> }[] = [];
const errors: string[] = [];

const codex = currentSettings.settings.providers.find((provider) => provider.id === 'codex');
const claude = providerCatalog.providers.find((provider) => provider.id === 'claude');
if (!codex || !claude) throw new Error('The app fixtures need Codex and Claude providers.');
currentSettings.settings.providers = [
  codex,
  {
    id: claude.id,
    enabled: true,
    detected: true,
    expanded: false,
    metrics: claude.metrics.map((metric) => ({
      id: metric.id,
      enabled: metric.defaultEnabled,
      section: metric.defaultSection,
      pinned: metric.defaultPinned,
    })),
  },
];

Object.defineProperty(window, 'innerHeight', {
  configurable: true,
  get: () => panelHeight,
});
window.__OPENQUOTA_E2E__ = {
  calls,
  errors,
  get panelHeight() {
    return panelHeight;
  },
  get panelHeightMode() {
    return panelHeightMode;
  },
};

mockWindows('main');
mockIPC(
  (command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    try {
      switch (command) {
        case 'get_bootstrap_state':
          return { usage: liveState, settings: currentSettings, catalog: providerCatalog };
        case 'get_app_settings':
          return currentSettings;
        case 'get_panel_resize_edge':
          return 'bottom';
        case 'get_panel_height_mode':
          return panelHeightMode;
        case 'get_panel_height_bounds':
          return { minimum: 240, maximum: 550 };
        case 'set_panel_height_manual':
          panelHeightMode = 'manual';
          if (typeof args.adjustment === 'number') {
            panelHeight = Math.min(550, Math.max(240, panelHeight + args.adjustment));
            window.dispatchEvent(new Event('resize'));
          }
          return;
        case 'set_panel_height_automatic':
          panelHeightMode = 'automatic';
          return;
        case 'fit_panel_to_content':
          return true;
        case 'save_app_settings': {
          if (typeof args.settings !== 'object' || args.settings === null) {
            throw new Error('save_app_settings requires settings.');
          }
          currentSettings = {
            ...currentSettings,
            settingsRevision: currentSettings.settingsRevision + 1,
            settings: args.settings as typeof currentSettings.settings,
          };
          return currentSettings;
        }
        case 'refresh_usage':
        case 'refresh_provider_usage':
          return liveState;
        case 'plugin:window|current_monitor':
          return {
            name: 'Playwright',
            position: { x: 0, y: 0 },
            size: { width: 1280, height: 900 },
            workArea: { position: { x: 0, y: 0 }, size: { width: 1280, height: 900 } },
            scaleFactor: 1,
          };
        case 'plugin:window|scale_factor':
          return 1;
        case 'plugin:window|inner_size':
          return { width: 640, height: panelHeight };
        case 'check_for_updates':
          return {
            available: false,
            currentVersion: '0.5.0',
            version: null,
            body: null,
            installable: true,
            releaseUrl: 'https://github.com/starise/OpenQuota/releases/latest',
          };
        default:
          throw new Error(`Unexpected Tauri IPC command: ${command}`);
      }
    } catch (error) {
      errors.push(`Tauri IPC ${command}: ${String(error)}`);
      throw error;
    }
  },
  { shouldMockEvents: true },
);

await import('../../src/main.ts');
