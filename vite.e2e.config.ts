import { mergeConfig } from 'vite';
import baseConfig from './vite.config.ts';

export default mergeConfig(baseConfig, {
  cacheDir: '.local/vite-e2e',
  server: { hmr: false },
});
