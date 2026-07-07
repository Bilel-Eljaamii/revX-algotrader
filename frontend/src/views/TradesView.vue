<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  trades,
  formatDateTime,
  formatCurrency,
  formatDuration,
  calculatePositions,
} from '../store';

const pageSize = ref(20);
const currentPage = ref(1);

// Filters
const filterAction = ref<'all' | 'long' | 'short'>('all');
const filterTimeRange = ref<string>('all');

// Sorting
type SortCol = 'time' | 'type' | 'pnl' | 'size' | 'duration';
const sortColumn = ref<SortCol>('time');
const sortDirection = ref<'asc' | 'desc'>('desc');

const closedPositions = computed(() => calculatePositions(trades.value));

const timeframeOptions = computed(() => {
  const months = new Map<string, string>();
  const weeks = new Map<string, string>();

  for (const pos of closedPositions.value) {
    const d = new Date(pos.exitTime);

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

const filteredAndSortedPositions = computed(() => {
  let result = closedPositions.value.slice();

  // 1. Filter
  if (filterAction.value !== 'all') {
    result = result.filter((p) => p.type === filterAction.value);
  }
  if (filterTimeRange.value !== 'all') {
    const [type, val] = filterTimeRange.value.split(':');
    result = result.filter((p) => {
      const d = new Date(p.exitTime);
      if (type === 'month') {
        const mKey = `${d.getFullYear()}-${(d.getMonth() + 1).toString().padStart(2, '0')}`;
        return mKey === val;
      } else if (type === 'week') {
        const day = d.getDay() || 7;
        const wStart = new Date(d);
        wStart.setDate(d.getDate() - (day - 1));
        wStart.setHours(0, 0, 0, 0);
        return String(wStart.getTime()) === val;
      }
      return true;
    });
  }

  // 2. Sort
  result.sort((a, b) => {
    let cmp = 0;
    if (sortColumn.value === 'time') {
      cmp = a.exitTime - b.exitTime;
    } else if (sortColumn.value === 'type') {
      cmp = a.type.localeCompare(b.type);
    } else if (sortColumn.value === 'pnl') {
      cmp = a.pnl - b.pnl;
    } else if (sortColumn.value === 'size') {
      cmp = a.size - b.size;
    } else if (sortColumn.value === 'duration') {
      cmp = a.durationMs - b.durationMs;
    }
    return sortDirection.value === 'asc' ? cmp : -cmp;
  });

  return result;
});

const totalPages = computed(() => {
  return Math.ceil(filteredAndSortedPositions.value.length / pageSize.value) || 1;
});

const paginatedPositions = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return filteredAndSortedPositions.value.slice(start, end);
});

const prevPage = () => {
  if (currentPage.value > 1) {
    currentPage.value--;
  }
};

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    currentPage.value++;
  }
};

const toggleSort = (col: SortCol) => {
  if (sortColumn.value === col) {
    sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc';
  } else {
    sortColumn.value = col;
    sortDirection.value = 'desc';
  }
};

const resetPage = () => {
  currentPage.value = 1;
};
</script>

<template>
  <div class="dashboard-grid">
    <section class="glass-panel animate-fade-in delay-100" style="grid-column: 1 / -1">
      <div
        style="
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 1rem;
        "
      >
        <h2>Closed Positions</h2>

        <!-- Filters -->
        <div
          style="
            display: flex;
            gap: 1rem;
            align-items: center;
            flex-wrap: wrap;
            justify-content: flex-end;
          "
        >
          <select v-model="filterTimeRange" class="filter-select" @change="resetPage">
            <option value="all">All Time</option>
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

          <select v-model="filterAction" class="filter-select" @change="resetPage">
            <option value="all">All Directions</option>
            <option value="long">Longs Only</option>
            <option value="short">Shorts Only</option>
          </select>

          <select v-model="pageSize" class="filter-select" @change="resetPage">
            <option :value="10">10 per page</option>
            <option :value="20">20 per page</option>
            <option :value="50">50 per page</option>
            <option :value="100">100 per page</option>
          </select>

          <span
            class="badge"
            style="background: rgba(255, 255, 255, 0.1); display: flex; align-items: center"
          >
            {{ filteredAndSortedPositions.length }} Positions
          </span>
        </div>
      </div>

      <div class="table-wrapper" style="max-height: 500px; overflow-y: auto">
        <table v-if="paginatedPositions.length > 0">
          <thead
            style="
              position: sticky;
              top: 0;
              z-index: 10;
              background: linear-gradient(180deg, #12141f 80%, rgba(18, 20, 31, 0) 100%);
            "
          >
            <tr>
              <th class="sortable-th" style="width: 20%" @click="toggleSort('time')">
                Close Time
                <span v-if="sortColumn === 'time'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
              <th class="sortable-th" style="width: 10%" @click="toggleSort('type')">
                Type
                <span v-if="sortColumn === 'type'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
              <th style="width: 10%">Symbol</th>
              <th style="text-align: right; width: 20%">Entry &rarr; Exit Price</th>
              <th
                class="sortable-th"
                style="text-align: right; width: 10%"
                @click="toggleSort('size')"
              >
                Size
                <span v-if="sortColumn === 'size'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
              <th
                class="sortable-th"
                style="text-align: right; width: 15%"
                @click="toggleSort('duration')"
              >
                Time Held
                <span v-if="sortColumn === 'duration'">{{
                  sortDirection === 'asc' ? '↑' : '↓'
                }}</span>
              </th>
              <th
                class="sortable-th"
                style="text-align: right; width: 15%; padding-right: 2rem"
                @click="toggleSort('pnl')"
              >
                Net PnL
                <span v-if="sortColumn === 'pnl'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="pos in paginatedPositions" :key="pos.id">
              <td class="text-tertiary" style="font-size: 0.85rem">
                {{ formatDateTime(pos.exitTime) }}
              </td>
              <td>
                <span
                  :class="['badge', pos.type === 'long' ? 'buy' : 'sell']"
                  style="text-transform: uppercase"
                  >{{ pos.type }}</span
                >
              </td>
              <td class="id-text">{{ pos.symbol }}</td>
              <td class="price-text" style="text-align: right; color: var(--text-primary)">
                <span style="opacity: 0.7">{{ pos.entryPrice.toFixed(4) }}</span>
                &rarr; {{ pos.exitPrice.toFixed(4) }}
              </td>
              <td class="price-text" style="text-align: right">
                {{ pos.size }}
              </td>
              <td class="text-tertiary" style="text-align: right; font-size: 0.85rem">
                {{ formatDuration(pos.durationMs) }}
              </td>
              <td
                style="text-align: right; padding-right: 2rem; font-weight: bold"
                :class="pos.pnl >= 0 ? 'text-success' : 'text-danger'"
              >
                {{ pos.pnl >= 0 ? '+' : '' }}{{ formatCurrency(pos.pnl) }}
                <span style="font-size: 0.75rem; opacity: 0.8; display: block; font-weight: normal">
                  ({{ pos.pnl >= 0 ? '+' : '' }}{{ pos.pnlPercent.toFixed(2) }}%)
                </span>
              </td>
            </tr>
          </tbody>
        </table>
        <div
          v-else
          class="text-muted"
          style="text-align: center; padding: 3rem; font-size: 0.95rem"
        >
          No completed positions match your filters.
        </div>
      </div>

      <!-- Pagination Controls -->
      <div
        v-if="totalPages > 1"
        style="
          display: flex;
          justify-content: center;
          align-items: center;
          gap: 1rem;
          margin-top: 1.5rem;
        "
      >
        <button :disabled="currentPage === 1" class="pagination-btn" @click="prevPage">
          Previous
        </button>
        <span class="text-tertiary" style="font-size: 0.85rem">
          Page
          <strong style="color: var(--text-primary)">{{ currentPage }}</strong>
          of {{ totalPages }}
        </span>
        <button :disabled="currentPage === totalPages" class="pagination-btn" @click="nextPage">
          Next
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.pagination-btn {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
  padding: 0.5rem 1rem;
  border-radius: 8px;
  font-family: var(--font-family);
  font-size: 0.85rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.pagination-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.pagination-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

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

.sortable-th {
  cursor: pointer;
  user-select: none;
  transition: color 0.2s;
  background: #12141f; /* Ensure background is solid for sticky overlap */
}

.sortable-th:hover {
  color: var(--text-primary);
}
</style>
