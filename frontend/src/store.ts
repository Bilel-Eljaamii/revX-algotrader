import { ref, computed } from 'vue';

export interface ApiTrade {
  id: string;
  orderId: string;
  symbol: string;
  side: string;
  price: number;
  size: number;
  timestamp: number;
}

export interface ClosedPosition {
  id: string;
  type: 'long' | 'short';
  symbol: string;
  size: number;
  entryPrice: number;
  exitPrice: number;
  entryTime: number;
  exitTime: number;
  durationMs: number;
  pnl: number;
  pnlPercent: number;
}

export interface ApiOrder {
  id: string;
  symbol: string;
  side: string;
  price: number;
  size: number;
  state: string;
  createdAt: number;
}

export const symbol = ref('USDC-USD');
export const allSymbols = ref<string[]>(['USDC-USD']);
export const apiStatus = ref<'connecting' | 'online' | 'offline'>('connecting');
export const trades = ref<ApiTrade[]>([]);
export const activeOrders = ref<ApiOrder[]>([]);
export const uptime = ref<number>(0);

export const currentPrice = ref<number>(1.0);
/* v8 ignore next 6: environment safety fallback for SSR/node contexts without DOM */
export const startingCapital = ref<number>(
  typeof localStorage !== 'undefined' && typeof localStorage.getItem === 'function'
    ? Number(localStorage.getItem('startCapital') || '1000')
    : 1000,
);

let pollInterval: ReturnType<typeof setInterval> | null = null;

export const fetchDashboardData = async () => {
  try {
    const health = await fetch('/health');
    if (health.ok) {
      apiStatus.value = 'online';
      const healthData = await health.json();
      uptime.value = healthData.uptime || 0;
    } else {
      apiStatus.value = 'offline';
    }

    // Fetch symbols
    const sRes = await fetch('/proxy/symbols');
    if (sRes.ok) {
      const sData = await sRes.json();
      if (sData && sData.data) allSymbols.value = sData.data;
    }

    // Fetch local trades from DB
    const tRes = await fetch(`/proxy/local-trades?symbol=${symbol.value}`);
    if (tRes.ok) {
      const tData = await tRes.json();
      if (tData && tData.data) {
        // Map Order model to ApiTrade
        trades.value = tData.data.map((o: any) => ({
          id: o.id,
          orderId: o.id,
          symbol: o.symbol,
          side: o.side.toLowerCase(),
          price: Number(o.avg_price || o.limit_price || 0),
          size: Number(o.filled_quantity || o.base_quantity || 0),
          timestamp: o.completed_at || o.updated_at || o.created_at || Date.now(),
        }));
      }
    }

    // Fetch active orders from Revolut X
    const oRes = await fetch(`/proxy/active-orders?symbols=${symbol.value}`);
    if (oRes.ok) {
      const oData = await oRes.json();
      if (oData && oData.data) {
        activeOrders.value = oData.data.map((o: any) => ({
          id: o.id,
          symbol: o.symbol,
          side: o.side.toLowerCase(),
          price: Number(o.limit_price || 0),
          size: Number(o.base_quantity || 0),
          state: o.status.toLowerCase(),
          createdAt: o.created_at || Date.now(),
        }));
      }
    }

    // Fetch current price from Revolut X
    const pRes = await fetch(`/proxy/ticker?symbol=${symbol.value}`);
    if (pRes.ok) {
      const pData = await pRes.json();
      if (pData && pData.data) {
        currentPrice.value = Number(pData.data.last_price || pData.data.mid || 0);
      }
    }
  } catch (err) {
    console.error('Fetch error:', err);
    apiStatus.value = 'offline';
  }
};

export const startPolling = () => {
  fetchDashboardData();
  if (!pollInterval) {
    pollInterval = setInterval(fetchDashboardData, 5000);
  }
};

export const stopPolling = () => {
  if (pollInterval) {
    clearInterval(pollInterval);
    pollInterval = null;
  }
};

export function formatTime(ts: number | string): string {
  if (!ts) return '-';
  const date = new Date(ts);
  return date.toLocaleTimeString(undefined, {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export function formatDateTime(ts: number | string): string {
  if (!ts) return '-';
  const date = new Date(ts);
  return date.toLocaleString(undefined, {
    weekday: 'short',
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export function formatDuration(ms: number): string {
  if (ms === 0) return 'Instant';
  const s = Math.floor(ms / 1000) % 60;
  const m = Math.floor(ms / 60000) % 60;
  const h = Math.floor(ms / 3600000);
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export function formatCurrency(val: number | string): string {
  const num = Number(val);
  if (isNaN(num)) return String(val);
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  }).format(num);
}

// --- Metrics Computations ---
export const activeBuys = computed(() => activeOrders.value.filter((o) => o.side === 'buy').length);
export const activeSells = computed(
  () => activeOrders.value.filter((o) => o.side === 'sell').length,
);

const executedBuys = computed(() => trades.value.filter((t) => t.side === 'buy'));
const executedSells = computed(() => trades.value.filter((t) => t.side === 'sell'));

export const totalBuyVolume = computed(() =>
  executedBuys.value.reduce((acc, t) => acc + t.price * t.size, 0),
);

export const totalSellVolume = computed(() =>
  executedSells.value.reduce((acc, t) => acc + t.price * t.size, 0),
);

// Net cash flow: Since grid trading is delta neutral ideally, selling brings cash (+), buying costs cash (-)
export const netCashflow = computed(() => totalSellVolume.value - totalBuyVolume.value);

// Inventory delta: Selling reduces inventory (-), Buying increases memory (+)
export const inventoryDelta = computed(
  () =>
    executedBuys.value.reduce((acc, t) => acc + t.size, 0) -
    executedSells.value.reduce((acc, t) => acc + t.size, 0),
);

export const avgBuyPrice = computed(() => {
  if (executedBuys.value.length === 0) return 0;
  return totalBuyVolume.value / executedBuys.value.reduce((acc, t) => acc + t.size, 0);
});

export const avgSellPrice = computed(() => {
  if (executedSells.value.length === 0) return 0;
  return totalSellVolume.value / executedSells.value.reduce((acc, t) => acc + t.size, 0);
});

export function calculatePositions(tradeList: ApiTrade[]): ClosedPosition[] {
  const positions: ClosedPosition[] = [];
  const openLongs: ApiTrade[] = [];
  const openShorts: ApiTrade[] = [];

  // Sort oldest first for natural pairings
  const sorted = [...tradeList].sort((a, b) => a.timestamp - b.timestamp);

  // We use standard FIFO netting:
  for (const t of sorted) {
    if (t.side === 'buy') {
      let remainingSize = t.size;
      // Close Shorts first
      while (remainingSize > 0 && openShorts.length > 0) {
        const oldestShort = openShorts[0];
        const matchSize = Math.min(oldestShort.size, remainingSize);
        const pnl = matchSize * (oldestShort.price - t.price);

        positions.push({
          id: t.id + oldestShort.id,
          type: 'short',
          symbol: t.symbol,
          size: matchSize,
          entryPrice: oldestShort.price,
          exitPrice: t.price,
          entryTime: oldestShort.timestamp,
          exitTime: t.timestamp,
          durationMs: t.timestamp - oldestShort.timestamp,
          pnl,
          pnlPercent: (pnl / (oldestShort.price * matchSize)) * 100,
        });

        oldestShort.size -= matchSize;
        remainingSize -= matchSize;
        if (oldestShort.size <= 0) openShorts.shift();
      }
      // Any remaining size opens a Long (if it wasn't covering a short)
      if (remainingSize > 0) {
        openLongs.push({ ...t, size: remainingSize });
      }
    } else if (t.side === 'sell') {
      let remainingSize = t.size;
      // Close Longs first
      while (remainingSize > 0 && openLongs.length > 0) {
        const oldestLong = openLongs[0];
        const matchSize = Math.min(oldestLong.size, remainingSize);
        const pnl = matchSize * (t.price - oldestLong.price);

        positions.push({
          id: t.id + oldestLong.id,
          type: 'long',
          symbol: t.symbol,
          size: matchSize,
          entryPrice: oldestLong.price,
          exitPrice: t.price,
          entryTime: oldestLong.timestamp,
          exitTime: t.timestamp,
          durationMs: t.timestamp - oldestLong.timestamp,
          pnl,
          pnlPercent: (pnl / (oldestLong.price * matchSize)) * 100,
        });

        oldestLong.size -= matchSize;
        remainingSize -= matchSize;
        if (oldestLong.size <= 0) openLongs.shift();
      }
      // Any remaining size opens a Short
      if (remainingSize > 0) {
        openShorts.push({ ...t, size: remainingSize });
      }
    }
  }

  return positions.sort((a, b) => b.exitTime - a.exitTime);
}

export function calculateRealizedPnL(tradeList: ApiTrade[]): number {
  const positions = calculatePositions(tradeList);
  return positions.reduce((acc, p) => acc + p.pnl, 0);
}
