/**
 * Comprehensive unit tests for all exported symbols from store.ts.
 *
 * Strategy:
 *  - Pure functions are tested directly.
 *  - Reactive refs/computeds are exercised by mutating `.value` in each test.
 *  - fetch() is mocked via vi.spyOn(globalThis, 'fetch').
 *  - Timers are replaced with vi.useFakeTimers() where needed.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  // state refs
  apiStatus,
  trades,
  activeOrders,
  uptime,
  currentPrice,
  startingCapital,
  symbol,
  // polling
  fetchDashboardData,
  startPolling,
  stopPolling,
  // formatters
  formatTime,
  formatDateTime,
  formatDuration,
  formatCurrency,
  // metrics computeds
  activeBuys,
  activeSells,
  totalBuyVolume,
  totalSellVolume,
  netCashflow,
  inventoryDelta,
  avgBuyPrice,
  avgSellPrice,
  // pnl engine
  calculatePositions,
  calculateRealizedPnL,
} from '../../src/store';
import type { ApiTrade, ApiOrder } from '../../src/store';

// ─── helpers ────────────────────────────────────────────────────────────────
let _id = 0;
beforeEach(() => {
  _id = 0;
  trades.value = [];
  activeOrders.value = [];
  apiStatus.value = 'connecting';
  uptime.value = 0;
  currentPrice.value = 1.0;
});

const makeTrade = (o: Partial<ApiTrade> = {}): ApiTrade => ({
  id: `t_${++_id}`,
  orderId: `o_${_id}`,
  symbol: 'USDC-USD',
  side: 'buy',
  price: 1.0,
  size: 100,
  timestamp: _id * 1000,
  ...o,
});

const makeOrder = (o: Partial<ApiOrder> = {}): ApiOrder => ({
  id: `ord_${++_id}`,
  symbol: 'USDC-USD',
  side: 'buy',
  price: 1.0,
  size: 100,
  state: 'new',
  createdAt: Date.now(),
  ...o,
});

// ─── fetchDashboardData ───────────────────────────────────────────────────────
describe('fetchDashboardData', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    stopPolling();
  });

  const mockFetch = (responses: Record<string, unknown>) => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      const key = url.toString();
      const matched = Object.entries(responses).find(([k]) => key.includes(k));
      if (!matched) throw new Error(`Unmocked URL: ${key}`);
      return {
        ok: true,
        json: async () => matched[1],
      } as Response;
    });
  };

  it('sets apiStatus to online when health returns ok', async () => {
    mockFetch({
      '/api/health': { status: 'ok', uptime: 42 },
      '/api/trades': { data: [] },
      '/api/orders': { data: [] },
      '/api/price': { data: 1.0003 },
    });
    await fetchDashboardData();
    expect(apiStatus.value).toBe('online');
    expect(uptime.value).toBe(42);
  });

  it('updates trades and orders from API responses', async () => {
    const fakeTrade = makeTrade({ side: 'sell' });
    const fakeOrder = makeOrder({ side: 'sell' });
    mockFetch({
      '/api/health': { status: 'ok', uptime: 10 },
      '/api/trades': { data: [fakeTrade] },
      '/api/orders': { data: [fakeOrder] },
      '/api/price': { data: 1.0002 },
    });
    await fetchDashboardData();
    expect(trades.value).toHaveLength(1);
    expect(activeOrders.value).toHaveLength(1);
    expect(currentPrice.value).toBe(1.0002);
  });

  it('sets apiStatus to offline when health endpoint is not ok', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      if (url.toString().includes('/api/health')) {
        return { ok: false, json: async () => ({}) } as Response;
      }
      return { ok: true, json: async () => ({}) } as Response;
    });
    await fetchDashboardData();
    expect(apiStatus.value).toBe('offline');
  });

  it('sets apiStatus to offline when fetch throws', async () => {
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network unreachable'));
    await fetchDashboardData();
    expect(apiStatus.value).toBe('offline');
  });

  it('does NOT update trades if response has no data field', async () => {
    trades.value = [makeTrade()]; // pre-existing
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      if (url.toString().includes('/api/health')) {
        return { ok: true, json: async () => ({ uptime: 0 }) } as Response;
      }
      // trades / orders / price all return responses without data
      return { ok: true, json: async () => ({}) } as Response;
    });
    await fetchDashboardData();
    // trades should remain unchanged
    expect(trades.value).toHaveLength(1);
  });

  it('does NOT update trades when trades endpoint returns ok=false', async () => {
    trades.value = [makeTrade()]; // pre-existing
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      const s = url.toString();
      if (s.includes('/api/health'))
        return { ok: true, json: async () => ({ uptime: 5 }) } as Response;
      if (s.includes('/api/trades')) return { ok: false, json: async () => ({}) } as Response;
      if (s.includes('/api/orders'))
        return { ok: true, json: async () => ({ data: [] }) } as Response;
      if (s.includes('/api/price'))
        return { ok: true, json: async () => ({ data: 1.0 }) } as Response;
      throw new Error(`Unmocked: ${s}`);
    });
    await fetchDashboardData();
    expect(trades.value).toHaveLength(1); // unchanged
  });

  it('does NOT update activeOrders when orders endpoint returns ok=false', async () => {
    activeOrders.value = [makeOrder()]; // pre-existing
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      const s = url.toString();
      if (s.includes('/api/health'))
        return { ok: true, json: async () => ({ uptime: 5 }) } as Response;
      if (s.includes('/api/trades'))
        return { ok: true, json: async () => ({ data: [] }) } as Response;
      if (s.includes('/api/orders')) return { ok: false, json: async () => ({}) } as Response;
      if (s.includes('/api/price'))
        return { ok: true, json: async () => ({ data: 1.0 }) } as Response;
      throw new Error(`Unmocked: ${s}`);
    });
    await fetchDashboardData();
    expect(activeOrders.value).toHaveLength(1); // unchanged
  });

  it('does NOT update currentPrice when price endpoint returns ok=false', async () => {
    currentPrice.value = 1.5; // pre-existing
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url: RequestInfo | URL) => {
      const s = url.toString();
      if (s.includes('/api/health'))
        return { ok: true, json: async () => ({ uptime: 5 }) } as Response;
      if (s.includes('/api/trades'))
        return { ok: true, json: async () => ({ data: [] }) } as Response;
      if (s.includes('/api/orders'))
        return { ok: true, json: async () => ({ data: [] }) } as Response;
      if (s.includes('/api/price')) return { ok: false, json: async () => ({}) } as Response;
      throw new Error(`Unmocked: ${s}`);
    });
    await fetchDashboardData();
    expect(currentPrice.value).toBe(1.5); // unchanged
  });

  it('uptime defaults to 0 when healthData.uptime is missing', async () => {
    mockFetch({
      '/api/health': { status: 'ok' }, // no uptime field
      '/api/trades': { data: [] },
      '/api/orders': { data: [] },
      '/api/price': { data: 1.0 },
    });
    await fetchDashboardData();
    expect(uptime.value).toBe(0);
  });
});

// ─── startPolling / stopPolling ───────────────────────────────────────────────
describe('startPolling / stopPolling', () => {
  afterEach(() => {
    stopPolling();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('startPolling immediately calls fetchDashboardData', async () => {
    vi.useFakeTimers();
    // Prevent actual fetch
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no-net'));
    startPolling();
    await vi.runAllTicks(); // flush the async call that started
    expect(globalThis.fetch).toHaveBeenCalled();
  });

  it('startPolling schedules a 5-second interval', () => {
    vi.useFakeTimers();
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no-net'));
    const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
    startPolling();
    expect(setIntervalSpy).toHaveBeenCalledWith(expect.any(Function), 5000);
  });

  it('calling startPolling twice does not create a second interval', () => {
    vi.useFakeTimers();
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no-net'));
    const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
    startPolling();
    startPolling(); // second call
    expect(setIntervalSpy).toHaveBeenCalledTimes(1);
  });

  it('stopPolling clears the interval', () => {
    vi.useFakeTimers();
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no-net'));
    const clearSpy = vi.spyOn(globalThis, 'clearInterval');
    startPolling();
    stopPolling();
    expect(clearSpy).toHaveBeenCalled();
  });

  it('stopPolling is safe to call when not polling', () => {
    expect(() => stopPolling()).not.toThrow();
  });
});

// ─── formatTime ──────────────────────────────────────────────────────────────
describe('formatTime', () => {
  it('returns "-" for 0', () => expect(formatTime(0)).toBe('-'));
  it('returns "-" for empty string', () => expect(formatTime('')).toBe('-'));
  it('formats timestamp to HH:MM:SS', () => {
    expect(formatTime(new Date('2024-03-01T08:00:00Z').getTime())).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
  it('accepts ISO string', () => {
    expect(formatTime('2024-01-01T12:30:00Z')).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
  it('returns non-dash for Date.now()', () => {
    expect(formatTime(Date.now())).not.toBe('-');
  });
});

// ─── formatDateTime ───────────────────────────────────────────────────────────
describe('formatDateTime', () => {
  it('returns "-" for 0', () => expect(formatDateTime(0)).toBe('-'));
  it('returns "-" for empty string', () => expect(formatDateTime('')).toBe('-'));
  it('includes the year for a known date', () => {
    expect(formatDateTime(new Date('2026-04-14T10:00:00Z').getTime())).toContain('2026');
  });
  it('accepts ISO string input', () => {
    expect(formatDateTime('2025-01-01T00:00:00Z')).not.toBe('-');
  });
  it('returns non-dash for Date.now()', () => {
    expect(formatDateTime(Date.now())).not.toBe('-');
  });
});

// ─── formatDuration ──────────────────────────────────────────────────────────
describe('formatDuration', () => {
  it('"Instant" for 0ms', () => expect(formatDuration(0)).toBe('Instant'));
  it('formats seconds only', () => expect(formatDuration(45_000)).toBe('45s'));
  it('formats minutes and seconds', () =>
    expect(formatDuration(5 * 60_000 + 12_000)).toBe('5m 12s'));
  it('formats exact minutes (zero seconds)', () =>
    expect(formatDuration(3 * 60_000)).toBe('3m 0s'));
  it('formats hours, minutes, seconds', () => {
    expect(formatDuration(2 * 3_600_000 + 15 * 60_000 + 30_000)).toBe('2h 15m 30s');
  });
  it('formats exactly 1 hour', () => expect(formatDuration(3_600_000)).toBe('1h 0m 0s'));
});

// ─── formatCurrency ──────────────────────────────────────────────────────────
describe('formatCurrency', () => {
  it('formats positive number', () => expect(formatCurrency(1000)).toBe('$1,000.00'));
  it('formats zero', () => expect(formatCurrency(0)).toBe('$0.00'));
  it('formats numeric string', () => expect(formatCurrency('99.99')).toBe('$99.99'));
  it('returns original on non-numeric string', () => expect(formatCurrency('bad')).toBe('bad'));
  it('preserves 6 decimals', () => expect(formatCurrency(1.000185)).toBe('$1.000185'));
  it('rounds beyond 6 decimals', () => expect(formatCurrency(1.9999999)).toBe('$2.00'));
  it('formats negative', () => expect(formatCurrency(-50.5)).toContain('50.5'));
});

// ─── Metrics computed properties ─────────────────────────────────────────────
describe('computed metrics', () => {
  beforeEach(() => {
    activeOrders.value = [];
    trades.value = [];
  });

  it('activeBuys counts buy orders', () => {
    activeOrders.value = [makeOrder({ side: 'buy' }), makeOrder({ side: 'sell' })];
    expect(activeBuys.value).toBe(1);
  });

  it('activeSells counts sell orders', () => {
    activeOrders.value = [makeOrder({ side: 'sell' }), makeOrder({ side: 'sell' })];
    expect(activeSells.value).toBe(2);
  });

  it('activeBuys / activeSells return 0 when no orders', () => {
    activeOrders.value = [];
    expect(activeBuys.value).toBe(0);
    expect(activeSells.value).toBe(0);
  });

  it('totalBuyVolume sums price*size for buys', () => {
    trades.value = [
      makeTrade({ side: 'buy', price: 1.0, size: 100 }),
      makeTrade({ side: 'buy', price: 2.0, size: 50 }),
      makeTrade({ side: 'sell', price: 1.0, size: 100 }), // excluded
    ];
    expect(totalBuyVolume.value).toBeCloseTo(200, 4);
  });

  it('totalSellVolume sums price*size for sells', () => {
    trades.value = [
      makeTrade({ side: 'sell', price: 1.0002, size: 100 }),
      makeTrade({ side: 'buy', price: 1.0, size: 100 }), // excluded
    ];
    expect(totalSellVolume.value).toBeCloseTo(100.02, 4);
  });

  it('netCashflow = totalSellVolume - totalBuyVolume', () => {
    trades.value = [
      makeTrade({ side: 'buy', price: 1.0, size: 100 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100 }),
    ];
    expect(netCashflow.value).toBeCloseTo(1.0, 4);
  });

  it('inventoryDelta = total buy size - total sell size', () => {
    trades.value = [makeTrade({ side: 'buy', size: 200 }), makeTrade({ side: 'sell', size: 100 })];
    expect(inventoryDelta.value).toBe(100);
  });

  it('avgBuyPrice returns weighted average', () => {
    trades.value = [
      makeTrade({ side: 'buy', price: 1.0, size: 100 }),
      makeTrade({ side: 'buy', price: 2.0, size: 100 }),
    ];
    expect(avgBuyPrice.value).toBeCloseTo(1.5, 4);
  });

  it('avgBuyPrice returns 0 when no buy trades', () => {
    trades.value = [];
    expect(avgBuyPrice.value).toBe(0);
  });

  it('avgSellPrice returns weighted average', () => {
    trades.value = [
      makeTrade({ side: 'sell', price: 1.0, size: 100 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100 }),
    ];
    expect(avgSellPrice.value).toBeCloseTo(1.005, 4);
  });

  it('avgSellPrice returns 0 when no sell trades', () => {
    trades.value = [];
    expect(avgSellPrice.value).toBe(0);
  });

  it('startingCapital defaults to 1000 when localStorage has no value', () => {
    expect(startingCapital.value).toBe(1000);
  });

  it('symbol ref defaults to USDC-USD', () => {
    expect(symbol.value).toBe('USDC-USD');
  });
});

// ─── calculatePositions ──────────────────────────────────────────────────────
describe('calculatePositions', () => {
  it('returns empty for empty list', () => expect(calculatePositions([])).toEqual([]));

  it('creates a LONG position for buy → sell', () => {
    const t = [
      makeTrade({ side: 'buy', price: 0.9998, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.0, timestamp: 2000 }),
    ];
    const [pos] = calculatePositions(t);
    expect(pos.type).toBe('long');
    expect(pos.pnl).toBeCloseTo(0.0002 * 100, 6);
    expect(pos.pnlPercent).toBeCloseTo((0.0002 / 0.9998) * 100, 4);
    expect(pos.durationMs).toBe(1000);
  });

  it('creates a SHORT position for sell → buy', () => {
    const t = [
      makeTrade({ side: 'sell', price: 1.0002, timestamp: 1000 }),
      makeTrade({ side: 'buy', price: 1.0, timestamp: 3000 }),
    ];
    const [pos] = calculatePositions(t);
    expect(pos.type).toBe('short');
    expect(pos.pnl).toBeCloseTo(0.0002 * 100, 6);
  });

  it('returns positions newest-first', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.0, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.01, timestamp: 2000 }),
      makeTrade({ side: 'buy', price: 1.0, timestamp: 3000 }),
      makeTrade({ side: 'sell', price: 1.02, timestamp: 4000 }),
    ];
    const pos = calculatePositions(t);
    expect(pos[0].exitTime).toBeGreaterThan(pos[1].exitTime);
  });

  it('handles FIFO partial fill correctly', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.0, size: 200, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100, timestamp: 2000 }),
    ];
    const pos = calculatePositions(t);
    expect(pos).toHaveLength(1);
    expect(pos[0].size).toBe(100);
  });

  it('handles FIFO spanning multiple buys', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.0, size: 50, timestamp: 1000 }),
      makeTrade({ side: 'buy', price: 1.0, size: 50, timestamp: 2000 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100, timestamp: 3000 }),
    ];
    expect(calculatePositions(t)).toHaveLength(2);
  });

  it('leaves unmatched buys open (no positions created)', () => {
    const t = [makeTrade({ side: 'buy' })];
    expect(calculatePositions(t)).toHaveLength(0);
  });

  it('leaves unmatched sells open (no positions created)', () => {
    const t = [makeTrade({ side: 'sell' })];
    expect(calculatePositions(t)).toHaveLength(0);
  });

  it('handles unsorted input by sorting internally', () => {
    const t = [
      makeTrade({ side: 'sell', price: 1.01, timestamp: 2000 }),
      makeTrade({ side: 'buy', price: 1.0, timestamp: 1000 }),
    ];
    expect(calculatePositions(t)).toHaveLength(1);
    expect(calculatePositions(t)[0].type).toBe('long');
  });

  it('handles SHORT partial fill: buy smaller than open short (short stays open)', () => {
    // Sell 200, then buy 100 → closes 100 of the short, 100 remains open
    const t = [
      makeTrade({ side: 'sell', price: 1.0002, size: 200, timestamp: 1000 }),
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 2000 }),
    ];
    const pos = calculatePositions(t);
    expect(pos).toHaveLength(1);
    expect(pos[0].type).toBe('short');
    expect(pos[0].size).toBe(100);
  });

  it('handles SHORT FIFO spanning multiple sells: one buy closes two shorts', () => {
    const t = [
      makeTrade({ side: 'sell', price: 1.0002, size: 50, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.0002, size: 50, timestamp: 2000 }),
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 3000 }),
    ];
    const pos = calculatePositions(t);
    expect(pos).toHaveLength(2);
    expect(pos.every((p) => p.type === 'short')).toBe(true);
  });

  it('ignores trades with unknown side (covers implicit else branch)', () => {
    // A trade with side !== 'buy' and !== 'sell' should be silently skipped
    const unknown = { ...makeTrade(), side: 'unknown' } as unknown as ApiTrade;
    const t = [
      makeTrade({ side: 'buy', price: 1.0, timestamp: 1000 }),
      unknown,
      makeTrade({ side: 'sell', price: 1.01, timestamp: 3000 }),
    ];
    const pos = calculatePositions(t);
    expect(pos).toHaveLength(1);
    expect(pos[0].type).toBe('long');
  });
});

// ─── calculateRealizedPnL ────────────────────────────────────────────────────
describe('calculateRealizedPnL', () => {
  it('returns 0 for empty list', () => expect(calculateRealizedPnL([])).toBe(0));

  it('returns 0 when only buys exist', () => {
    expect(calculateRealizedPnL([makeTrade({ side: 'buy' })])).toBe(0);
  });

  it('returns 0 when only sells exist', () => {
    expect(calculateRealizedPnL([makeTrade({ side: 'sell' })])).toBe(0);
  });

  it('calculates profit for a long trade', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100, timestamp: 2000 }),
    ];
    expect(calculateRealizedPnL(t)).toBeCloseTo(1.0, 4);
  });

  it('calculates profit for a short trade', () => {
    const t = [
      makeTrade({ side: 'sell', price: 1.01, size: 100, timestamp: 1000 }),
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 2000 }),
    ];
    expect(calculateRealizedPnL(t)).toBeCloseTo(1.0, 4);
  });

  it('accumulates pnl across multiple positions', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.01, size: 100, timestamp: 2000 }),
      makeTrade({ side: 'buy', price: 1.0, size: 100, timestamp: 3000 }),
      makeTrade({ side: 'sell', price: 1.02, size: 100, timestamp: 4000 }),
    ];
    expect(calculateRealizedPnL(t)).toBeCloseTo(3.0, 4);
  });

  it('calculates a loss correctly', () => {
    const t = [
      makeTrade({ side: 'buy', price: 1.01, size: 100, timestamp: 1000 }),
      makeTrade({ side: 'sell', price: 1.0, size: 100, timestamp: 2000 }),
    ];
    expect(calculateRealizedPnL(t)).toBeCloseTo(-1.0, 4);
  });
});
