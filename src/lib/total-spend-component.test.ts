import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { providerCatalogIndex, settingsState } from '../test/appFixtures';
import TotalSpend from './TotalSpend.svelte';
import type { UsageHistory } from './types';

const incompleteUsage: UsageHistory = {
  today: {
    tokens: 500,
    estimatedCostUsd: 0.03,
    costEstimated: true,
    estimateComplete: false,
    unknownModels: ['future-unpriced-model'],
  },
  yesterday: null,
  last30Days: null,
  daily: [],
  unknownModels: ['future-unpriced-model'],
};

afterEach(cleanup);

describe('Total Spend', () => {
  it('labels known partial USD while preserving its amount', () => {
    render(TotalSpend, {
      providers: [{ id: 'codex', usage: incompleteUsage }],
      settings: {
        ...settingsState.settings,
        totalSpendMetric: 'cost',
        totalSpendPeriod: 'today',
      },
      catalog: providerCatalogIndex,
      onChange: vi.fn(),
      onShare: vi.fn(),
    });

    expect(screen.getByText('partial dollars')).toBeVisible();
    expect(screen.getByText('$0.03 partial')).toBeVisible();
  });

  it('hides cost per million for incomplete pricing coverage', () => {
    render(TotalSpend, {
      providers: [{ id: 'codex', usage: incompleteUsage }],
      settings: {
        ...settingsState.settings,
        totalSpendMetric: 'costPerMillion',
        totalSpendPeriod: 'today',
      },
      catalog: providerCatalogIndex,
      onChange: vi.fn(),
      onShare: vi.fn(),
    });

    expect(screen.getByText('Cost/MTok unavailable: some usage is unpriced')).toBeVisible();
    expect(screen.queryByText('$60/MTok')).not.toBeInTheDocument();
  });
});
