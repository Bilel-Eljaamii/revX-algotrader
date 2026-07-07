<script setup lang="ts">
import { computed, ref } from 'vue';
import { Doughnut, Line } from 'vue-chartjs';
import { Chart as ChartJS, registerables } from 'chart.js';

import {
  symbol,
  activeOrders,
  uptime,
  trades,
  formatTime,
  formatCurrency,
  currentPrice,
  startingCapital,
  calculateRealizedPnL,
  calculatePositions,
} from '../store';

ChartJS.register(...registerables);

// Local override to persist capital changes
const updateCapital = (e: Event) => {
  const val = (e.target as HTMLInputElement).value;
  startingCapital.value = Number(val);
  localStorage.setItem('startCapital', val);
};

const filterAction = ref<'all' | 'buy' | 'sell'>('all');
const filterTimeRange = ref<string>('all');

const timeframeOptions = computed(() => {
  const months = new Map<string, string>();
  const weeks = new Map<string, string>();

  for (const t of trades.value) {
    const d = new Date(t.timestamp);

    // Extract Month
    const mKey = `${d.getFullYear()}-${(d.getMonth() + 1).toString().padStart(2, '0')}`;
    const mLabel = d.toLocaleString(undefined, {
      month: 'short',
      year: 'numeric',
    });
    if (!months.has(mKey)) months.set(mKey, mLabel);

    // Extract Week (Monday start)
    const day = d.getDay() || 7;
    const wStart = new Date(d);
    wStart.setDate(d.getDate() - (day - 1));
    wStart.setHours(0, 0, 0, 0);
    const wKey = String(wStart.getTime());
    const wLabel =
      'Week of ' +
      wStart.toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      });
    if (!weeks.has(wKey)) weeks.set(wKey, wLabel);
  }

  return {
    months: Array.from(months.entries()).sort((a, b) => b[0].localeCompare(a[0])),
    weeks: Array.from(weeks.entries()).sort((a, b) => Number(b[0]) - Number(a[0])),
  };
});

const filteredTrades = computed(() => {
  return trades.value.filter((t) => {
    if (filterAction.value !== 'all' && t.side !== filterAction.value) return false;

    if (filterTimeRange.value !== 'all') {
      const [type, val] = filterTimeRange.value.split(':');
      const d = new Date(t.timestamp);
      if (type === 'month') {
        const mKey = `${d.getFullYear()}-${(d.getMonth() + 1).toString().padStart(2, '0')}`;
        if (mKey !== val) return false;
      } else if (type === 'week') {
        const day = d.getDay() || 7;
        const wStart = new Date(d);
        wStart.setDate(d.getDate() - (day - 1));
        wStart.setHours(0, 0, 0, 0);
        if (String(wStart.getTime()) !== val) return false;
      }
    }
    return true;
  });
});

const filteredOrders = computed(() => {
  return activeOrders.value.filter((o) => {
    if (filterAction.value !== 'all' && o.side !== filterAction.value) return false;
    return true;
  });
});

// Metrics
const activeBuys = computed(() => filteredOrders.value.filter((o) => o.side === 'buy').length);
const activeSells = computed(() => filteredOrders.value.filter((o) => o.side === 'sell').length);

const executedBuys = computed(() => filteredTrades.value.filter((t) => t.side === 'buy'));
const executedSells = computed(() => filteredTrades.value.filter((t) => t.side === 'sell'));

const totalBuyVolume = computed(() =>
  executedBuys.value.reduce((acc, t) => acc + t.price * t.size, 0),
);
const totalSellVolume = computed(() =>
  executedSells.value.reduce((acc, t) => acc + t.price * t.size, 0),
);
const netCashflow = computed(() => totalSellVolume.value - totalBuyVolume.value);
const inventoryDelta = computed(
  () =>
    executedBuys.value.reduce((acc, t) => acc + t.size, 0) -
    executedSells.value.reduce((acc, t) => acc + t.size, 0),
);

const avgBuyPrice = computed(() => {
  if (executedBuys.value.length === 0) return 0;
  return totalBuyVolume.value / executedBuys.value.reduce((acc, t) => acc + t.size, 0);
});

const avgSellPrice = computed(() => {
  if (executedSells.value.length === 0) return 0;
  return totalSellVolume.value / executedSells.value.reduce((acc, t) => acc + t.size, 0);
});

// PnL Computations
const stringentRealizedPnL = computed(() => calculateRealizedPnL(filteredTrades.value));

const unrealizedPnL = computed(() => {
  if (inventoryDelta.value <= 0) return 0;
  // Cost of current inventory based on avgBuyPrice
  const inventoryCost = inventoryDelta.value * avgBuyPrice.value;
  // Current market value of inventory
  const inventoryValue = inventoryDelta.value * currentPrice.value;
  return inventoryValue - inventoryCost;
});

const totalPnL = computed(() => stringentRealizedPnL.value + unrealizedPnL.value);

const roiPercentage = computed(() => {
  if (startingCapital.value <= 0) return 0;
  return (totalPnL.value / startingCapital.value) * 100;
});

const closedPositions = computed(() => calculatePositions(filteredTrades.value));

const profitabilityStats = computed(() => {
  const positions = closedPositions.value;
  if (!positions.length) return null;

  const dailyPnL = new Map<string, number>();
  const weeklyPnL = new Map<string, number>();
  const monthlyPnL = new Map<string, number>();

  for (const pos of positions) {
    const d = new Date(pos.exitTime);

    // Day
    const dayKey = d.toLocaleString(undefined, {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
    dailyPnL.set(dayKey, (dailyPnL.get(dayKey) || 0) + pos.pnl);

    // Week
    const day = d.getDay() || 7;
    const wStart = new Date(d);
    wStart.setDate(d.getDate() - (day - 1));
    const wKey = 'Week of ' + wStart.toLocaleString(undefined, { month: 'short', day: 'numeric' });
    weeklyPnL.set(wKey, (weeklyPnL.get(wKey) || 0) + pos.pnl);

    // Month
    const mKey = d.toLocaleString(undefined, {
      month: 'long',
      year: 'numeric',
    });
    monthlyPnL.set(mKey, (monthlyPnL.get(mKey) || 0) + pos.pnl);
  }

  const getExtremes = (map: Map<string, number>) => {
    if (map.size === 0) return { best: { key: '-', val: 0 }, worst: { key: '-', val: 0 } };
    let best = { key: '-', val: -Infinity };
    let worst = { key: '-', val: Infinity };
    for (const [key, val] of map.entries()) {
      if (val > best.val) best = { key, val };
      if (val < worst.val) worst = { key, val };
    }
    return { best, worst };
  };

  return {
    day: getExtremes(dailyPnL),
    week: getExtremes(weeklyPnL),
    month: getExtremes(monthlyPnL),
  };
});

// Chart Data: Trade Distribution
const distributionChartData = computed(() => {
  const buys = filteredTrades.value.filter((t) => t.side === 'buy').length;
  const sells = filteredTrades.value.filter((t) => t.side === 'sell').length;

  return {
    labels: ['Buys', 'Sells'],
    datasets: [
      {
        backgroundColor: ['#00f2fe', '#f53d5c'],
        data: [buys, sells],
        borderWidth: 0,
        hoverOffset: 10,
      },
    ],
  };
});

const distributionChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: { display: false },
  },
  cutout: '70%',
};

// Chart Data: Price Performance
const performanceChartData = computed(() => {
  const lastTrades = filteredTrades.value.slice(-15);
  return {
    labels: lastTrades.map((t) => formatTime(t.timestamp)),
    datasets: [
      {
        label: 'Execution Price',
        backgroundColor: 'rgba(0, 242, 254, 0.1)',
        borderColor: '#00f2fe',
        data: lastTrades.map((t) => t.price),
        tension: 0.4,
        fill: true,
        pointBackgroundColor: '#00f2fe',
        pointBorderColor: '#fff',
        pointHoverRadius: 6,
      },
    ],
  };
});

const performanceChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  scales: {
    y: {
      grid: { color: 'rgba(255, 255, 255, 0.05)' },
      ticks: { color: 'rgba(255, 255, 255, 0.5)', font: { size: 10 } },
    },
    x: {
      grid: { display: false },
      ticks: { color: 'rgba(255, 255, 255, 0.5)', font: { size: 10 } },
    },
  },
  plugins: {
    legend: { display: false },
    tooltip: {
      backgroundColor: 'rgba(10, 10, 20, 0.9)',
      titleColor: '#fff',
      bodyColor: '#00f2fe',
      borderColor: 'rgba(0, 242, 254, 0.2)',
      borderWidth: 1,
      padding: 10,
      displayColors: false,
    },
  },
};
</script>

<template>
  <div class="dashboard-grid">
    <!-- Filters Header Component -->
    <section class="glass-panel animate-fade-in" style="grid-column: 1 / -1; padding: 1rem 2rem">
      <div
        style="
          display: flex;
          justify-content: space-between;
          align-items: center;
          flex-wrap: wrap;
          gap: 1rem;
        "
      >
        <h2 style="margin: 0; font-size: 1.25rem">Dashboard Config</h2>
        <div style="display: flex; gap: 1rem; align-items: center">
          <div style="display: flex; flex-direction: column">
            <label style="font-size: 0.75rem; color: var(--text-tertiary); margin-bottom: 0.25rem"
              >TIME FILTER</label
            >
            <select v-model="filterTimeRange" class="filter-select">
              <option value="all">Global (All Time)</option>
              <optgroup label="By Month">
                <option
                  v-for="[key, label] in timeframeOptions.months"
                  :key="'m' + key"
                  :value="'month:' + key"
                >
                  {{ label }}
                </option>
              </optgroup>
              <optgroup label="By Week">
                <option
                  v-for="[key, label] in timeframeOptions.weeks"
                  :key="'w' + key"
                  :value="'week:' + key"
                >
                  {{ label }}
                </option>
              </optgroup>
            </select>
          </div>

          <div style="display: flex; flex-direction: column">
            <label style="font-size: 0.75rem; color: var(--text-tertiary); margin-bottom: 0.25rem"
              >ACTION FILTER</label
            >
            <select v-model="filterAction" class="filter-select">
              <option value="all">All Actions</option>
              <option value="buy">Buy Only</option>
              <option value="sell">Sell Only</option>
            </select>
          </div>

          <div style="display: flex; flex-direction: column">
            <label style="font-size: 0.75rem; color: var(--text-tertiary); margin-bottom: 0.25rem"
              >INITIAL CAP (USD)</label
            >
            <input
              type="number"
              class="filter-select"
              :value="startingCapital"
              style="width: 120px"
              @input="updateCapital"
            />
          </div>
        </div>
      </div>
    </section>

    <!-- Performance & PnL Header -->
    <section
      class="glass-panel animate-fade-in delay-50"
      style="
        grid-column: 1 / -1;
        display: flex;
        justify-content: space-around;
        padding: 1.5rem;
        background: linear-gradient(135deg, rgba(20, 22, 35, 0.8), rgba(15, 18, 30, 0.9));
        border-top: 2px solid var(--accent-primary);
      "
    >
      <div style="text-align: center">
        <div class="text-xs" style="color: var(--text-secondary)">Realized PnL</div>
        <div
          :class="stringentRealizedPnL >= 0 ? 'text-success' : 'text-danger'"
          style="font-size: 1.75rem; font-weight: bold"
        >
          {{ stringentRealizedPnL >= 0 ? '+' : '' }}{{ formatCurrency(stringentRealizedPnL) }}
        </div>
      </div>
      <div style="text-align: center">
        <div class="text-xs" style="color: var(--text-secondary)">Unrealized (Floating)</div>
        <div
          :class="unrealizedPnL >= 0 ? 'text-success' : 'text-danger'"
          style="font-size: 1.75rem; font-weight: bold"
        >
          {{ unrealizedPnL >= 0 ? '+' : '' }}{{ formatCurrency(unrealizedPnL) }}
        </div>
      </div>
      <div style="text-align: center">
        <div class="text-xs" style="color: var(--text-secondary)">Total PnL</div>
        <div
          :class="totalPnL >= 0 ? 'text-primary' : 'text-danger'"
          style="
            font-size: 1.75rem;
            font-weight: bold;
            text-shadow: 0 0 10px rgba(0, 242, 254, 0.3);
          "
        >
          {{ totalPnL >= 0 ? '+' : '' }}{{ formatCurrency(totalPnL) }}
        </div>
      </div>
      <div style="text-align: center">
        <div class="text-xs" style="color: var(--text-secondary)">ROI</div>
        <div
          :class="roiPercentage >= 0 ? 'text-success' : 'text-danger'"
          style="font-size: 1.75rem; font-weight: bold"
        >
          {{ roiPercentage >= 0 ? '+' : '' }}{{ roiPercentage.toFixed(2) }}%
        </div>
      </div>
    </section>

    <!-- Active Params & Distribution -->
    <section class="glass-panel animate-fade-in delay-100" style="display: flex; gap: 2rem">
      <div style="flex: 1">
        <div style="display: flex; justify-content: space-between; align-items: center">
          <h2>Strategy State</h2>
          <span class="badge new" style="letter-spacing: 2px">{{ symbol }}</span>
        </div>

        <div class="stats-grid">
          <div class="stat-card">
            <div class="text-xs">Active Buys</div>
            <div class="stat-value text-success">{{ activeBuys }}</div>
          </div>
          <div class="stat-card">
            <div class="text-xs">Active Sells</div>
            <div class="stat-value text-danger">{{ activeSells }}</div>
          </div>
          <div class="stat-card">
            <div class="text-xs">Total Orders</div>
            <div class="stat-value stat-value-accent" style="font-size: 1.5rem">
              {{ filteredOrders.length }}
            </div>
          </div>
          <div class="stat-card">
            <div class="text-xs">Uptime</div>
            <div
              class="stat-value text-primary"
              style="font-size: 1.5rem; color: var(--text-primary)"
            >
              {{ Math.floor(uptime / 60) }}m {{ Math.floor(uptime % 60) }}s
            </div>
          </div>
        </div>
      </div>

      <div v-if="filteredTrades.length > 0" style="width: 140px; height: 140px; position: relative">
        <Doughnut :data="distributionChartData" :options="distributionChartOptions" />
        <div
          style="
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            text-align: center;
          "
        >
          <div style="font-size: 0.7rem; color: rgba(255, 255, 255, 0.5)">Ratio</div>
          <div style="font-weight: bold; font-size: 0.9rem">
            {{ filteredTrades.length }}
          </div>
        </div>
      </div>
    </section>

    <!-- Detailed Financial Metrics -->
    <section class="glass-panel animate-fade-in delay-200">
      <div style="display: flex; justify-content: space-between; align-items: center">
        <h2>Financial Metrics</h2>
        <span class="badge" style="background: rgba(255, 255, 255, 0.1)">Cumulative</span>
      </div>

      <div class="stats-grid" style="grid-template-columns: repeat(2, 1fr)">
        <div class="stat-card">
          <div class="text-xs">Net Cashflow (USD)</div>
          <div
            class="stat-value"
            :class="netCashflow >= 0 ? 'text-success' : 'text-danger'"
            style="font-size: 1.25rem"
          >
            {{ netCashflow > 0 ? '+' : '' }}{{ formatCurrency(netCashflow) }}
          </div>
        </div>
        <div class="stat-card">
          <div class="text-xs">Inventory Delta</div>
          <div
            class="stat-value"
            :class="inventoryDelta >= 0 ? 'text-primary' : 'text-danger'"
            style="font-size: 1.25rem"
          >
            {{ inventoryDelta > 0 ? '+' : '' }}{{ inventoryDelta.toFixed(4) }}
          </div>
        </div>
        <div class="stat-card">
          <div class="text-xs">Avg Buy Price</div>
          <div class="stat-value text-primary" style="font-size: 1.25rem">
            {{ formatCurrency(avgBuyPrice) }}
          </div>
        </div>
        <div class="stat-card">
          <div class="text-xs">Avg Sell Price</div>
          <div class="stat-value text-primary" style="font-size: 1.25rem">
            {{ formatCurrency(avgSellPrice) }}
          </div>
        </div>
        <div class="stat-card">
          <div class="text-xs">Total Buy Vol</div>
          <div class="stat-value text-muted" style="font-size: 1rem">
            {{ formatCurrency(totalBuyVolume) }}
          </div>
        </div>
        <div class="stat-card">
          <div class="text-xs">Total Sell Vol</div>
          <div class="stat-value text-muted" style="font-size: 1rem">
            {{ formatCurrency(totalSellVolume) }}
          </div>
        </div>
      </div>
    </section>

    <!-- Period Profitability Extremes -->
    <section
      v-if="profitabilityStats"
      class="glass-panel animate-fade-in delay-250"
      style="grid-column: 1 / -1"
    >
      <div
        style="
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 1rem;
        "
      >
        <h2>Market Period Extremes</h2>
        <span class="badge" style="background: rgba(255, 255, 255, 0.1)">Extremes</span>
      </div>

      <div class="stats-grid" style="grid-template-columns: repeat(3, 1fr)">
        <div class="stat-card" style="border-left: 3px solid var(--success-color)">
          <div class="text-xs">Most Profitable Day</div>
          <div class="stat-value text-success" style="font-size: 1.1rem">
            +{{ formatCurrency(profitabilityStats.day.best.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.day.best.key }}
          </div>
        </div>
        <div class="stat-card" style="border-left: 3px solid var(--success-color)">
          <div class="text-xs">Most Profitable Week</div>
          <div class="stat-value text-success" style="font-size: 1.1rem">
            +{{ formatCurrency(profitabilityStats.week.best.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.week.best.key }}
          </div>
        </div>
        <div class="stat-card" style="border-left: 3px solid var(--success-color)">
          <div class="text-xs">Most Profitable Month</div>
          <div class="stat-value text-success" style="font-size: 1.1rem">
            +{{ formatCurrency(profitabilityStats.month.best.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.month.best.key }}
          </div>
        </div>

        <div class="stat-card" style="border-left: 3px solid var(--danger-color)">
          <div class="text-xs">Least Profitable Day</div>
          <div class="stat-value text-danger" style="font-size: 1.1rem">
            {{ formatCurrency(profitabilityStats.day.worst.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.day.worst.key }}
          </div>
        </div>
        <div class="stat-card" style="border-left: 3px solid var(--danger-color)">
          <div class="text-xs">Least Profitable Week</div>
          <div class="stat-value text-danger" style="font-size: 1.1rem">
            {{ formatCurrency(profitabilityStats.week.worst.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.week.worst.key }}
          </div>
        </div>
        <div class="stat-card" style="border-left: 3px solid var(--danger-color)">
          <div class="text-xs">Least Profitable Month</div>
          <div class="stat-value text-danger" style="font-size: 1.1rem">
            {{ formatCurrency(profitabilityStats.month.worst.val) }}
          </div>
          <div
            class="text-xs text-tertiary"
            style="
              margin-top: 0.2rem;
              white-space: nowrap;
              overflow: hidden;
              text-overflow: ellipsis;
            "
          >
            {{ profitabilityStats.month.worst.key }}
          </div>
        </div>
      </div>
    </section>

    <!-- Performance Chart -->
    <section
      class="glass-panel animate-fade-in delay-300"
      style="grid-column: 1 / -1; height: 320px"
    >
      <h2>Execution Price Performance</h2>
      <div style="height: calc(100% - 40px)">
        <Line
          v-if="filteredTrades.length > 0"
          :data="performanceChartData"
          :options="performanceChartOptions"
        />
        <div
          v-else
          style="
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100%;
            color: rgba(255, 255, 255, 0.3);
            font-size: 0.9rem;
          "
        >
          Insufficient trade data for performance visualization.
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.filter-select {
  background: rgba(18, 20, 31, 0.8);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 0.4rem 0.8rem;
  border-radius: 8px;
  font-family: var(--font-family);
  font-size: 0.85rem;
  outline: none;
  cursor: pointer;
}

.filter-select:hover {
  border-color: rgba(255, 255, 255, 0.2);
}

.filter-select option {
  background: #05050a;
  color: #fff;
}
</style>
