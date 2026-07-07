<script setup lang="ts">
import { ref, computed } from 'vue';
import { activeOrders, formatCurrency } from '../store';

const pageSize = ref(20);
const currentPage = ref(1);

// Filters
const filterAction = ref<'all' | 'buy' | 'sell'>('all');

// Sorting
type SortCol = 'action' | 'status' | 'price' | 'size';
const sortColumn = ref<SortCol>('price'); // Changed default sort to price since orders don't display time right now
const sortDirection = ref<'asc' | 'desc'>('desc');

const filteredAndSortedOrders = computed(() => {
  let result = activeOrders.value.slice();

  if (filterAction.value !== 'all') {
    result = result.filter((o) => o.side === filterAction.value);
  }

  // 2. Sort
  result.sort((a, b) => {
    let cmp = 0;
    if (sortColumn.value === 'action') {
      cmp = a.side.localeCompare(b.side);
    } else if (sortColumn.value === 'status') {
      const aState = a.state || 'PENDING';
      const bState = b.state || 'PENDING';
      cmp = aState.localeCompare(bState);
    } else if (sortColumn.value === 'price') {
      cmp = a.price - b.price;
    } else if (sortColumn.value === 'size') {
      cmp = a.size - b.size;
    }
    return sortDirection.value === 'asc' ? cmp : -cmp;
  });

  return result;
});

const totalPages = computed(() => {
  return Math.ceil(filteredAndSortedOrders.value.length / pageSize.value) || 1;
});

const paginatedOrders = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return filteredAndSortedOrders.value.slice(start, end);
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
        <h2>Pending Orders</h2>

        <!-- Filters -->
        <div style="display: flex; gap: 1rem; align-items: center">
          <select v-model="filterAction" class="filter-select" @change="resetPage">
            <option value="all">All Actions</option>
            <option value="buy">Buy</option>
            <option value="sell">Sell</option>
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
            {{ filteredAndSortedOrders.length }} Active
          </span>
        </div>
      </div>

      <div class="table-wrapper" style="max-height: 500px; overflow-y: auto">
        <table v-if="paginatedOrders.length > 0">
          <thead style="position: sticky; top: 0; z-index: 10; background: #12141f">
            <tr>
              <th class="sortable-th" style="width: 15%" @click="toggleSort('action')">
                Action
                <span v-if="sortColumn === 'action'">{{
                  sortDirection === 'asc' ? '↑' : '↓'
                }}</span>
              </th>
              <th style="width: 15%">Symbol</th>
              <th class="sortable-th" style="width: 15%" @click="toggleSort('status')">
                Status
                <span v-if="sortColumn === 'status'">{{
                  sortDirection === 'asc' ? '↑' : '↓'
                }}</span>
              </th>
              <th
                class="sortable-th"
                style="text-align: right; width: 20%"
                @click="toggleSort('price')"
              >
                Price
                <span v-if="sortColumn === 'price'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
              <th
                class="sortable-th"
                style="text-align: right; width: 15%"
                @click="toggleSort('size')"
              >
                Size
                <span v-if="sortColumn === 'size'">{{ sortDirection === 'asc' ? '↑' : '↓' }}</span>
              </th>
              <th style="padding-left: 2rem">Order ID</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="order in paginatedOrders" :key="order.id">
              <td>
                <span :class="['badge', order.side]">{{ order.side }}</span>
              </td>
              <td class="id-text">{{ order.symbol }}</td>
              <td>
                <span class="badge filled">{{ order.state || 'PENDING' }}</span>
              </td>
              <td class="price-text" style="text-align: right; color: var(--text-primary)">
                {{ formatCurrency(order.price) }}
              </td>
              <td class="price-text" style="text-align: right">
                {{ order.size }}
              </td>
              <td class="id-text" style="padding-left: 2rem">{{ order.id.substring(0, 13) }}...</td>
            </tr>
          </tbody>
        </table>
        <div
          v-else
          class="text-muted"
          style="text-align: center; padding: 3rem; font-size: 0.95rem"
        >
          No active orders in the current cycle match your filters.
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
  background: #12141f;
}

.sortable-th:hover {
  color: var(--text-primary);
}
</style>
