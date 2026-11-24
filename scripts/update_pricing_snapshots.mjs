// Regenerates the bundled LiteLLM and models.dev pricing snapshots.
// Requires Node.js 22+. Run with: node scripts/update_pricing_snapshots.mjs

import { readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const resourcesDirectory = join(scriptDirectory, '..', 'src-tauri', 'resources');
const feeds = {
  litellm:
    'https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json',
  modelsDev: 'https://models.dev/api.json',
};
const requestTimeoutMs = 120_000;

function isRecord(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function number(value) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function compactModel(input, output, cacheWrite, cacheRead, optional = {}) {
  const model = { i: input, o: output, cw: cacheWrite, cr: cacheRead };
  for (const [key, value] of Object.entries(optional)) {
    if (value !== null) model[key] = value;
  }
  return model;
}

function parseLiteLlm(data) {
  if (!isRecord(data)) throw new Error('LiteLLM feed must be a JSON object.');

  const models = Object.create(null);
  for (const [modelId, entry] of Object.entries(data)) {
    if (!isRecord(entry)) continue;

    const input = number(entry.input_cost_per_token);
    const output = number(entry.output_cost_per_token);
    if (input === null || output === null) continue;

    const providerSpecific = isRecord(entry.provider_specific_entry)
      ? entry.provider_specific_entry
      : {};
    const cacheWrite = number(entry.cache_creation_input_token_cost);
    const cacheRead = number(entry.cache_read_input_token_cost);
    const inputAbove200k = number(entry.input_cost_per_token_above_200k_tokens);
    const outputAbove200k = number(entry.output_cost_per_token_above_200k_tokens);
    const cacheWriteAbove200k = number(
      entry.cache_creation_input_token_cost_above_200k_tokens,
    );
    const cacheReadAbove200k = number(entry.cache_read_input_token_cost_above_200k_tokens);

    models[modelId] = compactModel(
      input * 1e6,
      output * 1e6,
      (cacheWrite ?? input) * 1e6,
      (cacheRead ?? input * 0.1) * 1e6,
      {
        ia: inputAbove200k === null ? null : inputAbove200k * 1e6,
        oa: outputAbove200k === null ? null : outputAbove200k * 1e6,
        cwa: cacheWriteAbove200k === null ? null : cacheWriteAbove200k * 1e6,
        cra: cacheReadAbove200k === null ? null : cacheReadAbove200k * 1e6,
        fast: number(providerSpecific.fast),
      },
    );
  }

  if (Object.keys(models).length === 0) {
    throw new Error('LiteLLM feed produced no usable entries.');
  }
  return models;
}

function parseModelsDev(data) {
  if (!isRecord(data)) throw new Error('models.dev feed must be a JSON object.');

  const models = Object.create(null);
  for (const providerName of Object.keys(data).sort()) {
    const provider = data[providerName];
    if (!isRecord(provider) || !isRecord(provider.models)) continue;

    for (const [modelId, model] of Object.entries(provider.models)) {
      if (Object.hasOwn(models, modelId) || !isRecord(model)) continue;
      const cost = isRecord(model.cost) ? model.cost : {};
      const input = number(cost.input);
      const output = number(cost.output);
      if (input === null || output === null) continue;

      const cacheWrite = number(cost.cache_write);
      const cacheRead = number(cost.cache_read);
      models[modelId] = compactModel(
        input,
        output,
        cacheWrite ?? input,
        cacheRead ?? input * 0.1,
      );
    }
  }

  if (Object.keys(models).length === 0) {
    throw new Error('models.dev feed produced no usable entries.');
  }
  return models;
}

function sameCatalog(current, next) {
  if (!isRecord(current)) return false;
  const currentIds = Object.keys(current).sort();
  const nextIds = Object.keys(next).sort();
  if (currentIds.length !== nextIds.length) return false;

  return currentIds.every((modelId, index) => {
    if (modelId !== nextIds[index] || !isRecord(current[modelId])) return false;
    const currentRates = current[modelId];
    const nextRates = next[modelId];
    const currentKeys = Object.keys(currentRates).sort();
    const nextKeys = Object.keys(nextRates).sort();
    return (
      currentKeys.length === nextKeys.length &&
      currentKeys.every(
        (key, rateIndex) => key === nextKeys[rateIndex] && currentRates[key] === nextRates[key],
      )
    );
  });
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  if (isRecord(value)) {
    const fields = Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`);
    return `{${fields.join(',')}}`;
  }
  return JSON.stringify(value);
}

async function writeSnapshot(filename, models, retrievedAt) {
  const path = join(resourcesDirectory, filename);
  try {
    const current = JSON.parse(await readFile(path, 'utf8'));
    if (sameCatalog(current.models, models)) {
      console.log(`${filename}: unchanged (${Object.keys(models).length} models)`);
      return;
    }
  } catch (error) {
    if (error?.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error;
  }

  await writeFile(path, stableJson({ models, retrieved_at: retrievedAt }));
  console.log(`${filename}: ${Object.keys(models).length} models`);
}

async function fetchJson(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(requestTimeoutMs) });
  if (!response.ok) throw new Error(`Pricing feed returned HTTP ${response.status}: ${url}`);
  return response.json();
}

async function main() {
  const [liteLlmData, modelsDevData] = await Promise.all([
    fetchJson(feeds.litellm),
    fetchJson(feeds.modelsDev),
  ]);
  const liteLlmModels = parseLiteLlm(liteLlmData);
  const modelsDevModels = parseModelsDev(modelsDevData);
  const retrievedAt = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');

  await writeSnapshot('pricing_litellm_snapshot.json', liteLlmModels, retrievedAt);
  await writeSnapshot('pricing_models_dev_snapshot.json', modelsDevModels, retrievedAt);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
