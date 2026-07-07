<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { apiStatus, startPolling, stopPolling, symbol, allSymbols } from './store';

onMounted(() => {
  startPolling();
});

onUnmounted(() => {
  stopPolling();
});
</script>

<template>
  <div
    style="
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      margin-bottom: 2rem;
    "
    class="animate-fade-in"
  >
    <header>
      <h1>RevX</h1>
      <h2>High-Performance Trading Suite</h2>
    </header>
    <div style="display: flex; align-items: center; gap: 1.5rem">
      <div class="symbol-selector">
        <label for="symbol-select">SYMBOL</label>
        <select id="symbol-select" v-model="symbol">
          <option v-for="s in allSymbols" :key="s" :value="s">{{ s }}</option>
        </select>
      </div>
      <div class="status-indicator">
        <div class="pulse-dot" :class="apiStatus"></div>
        <span>{{
          apiStatus === 'online'
            ? 'System Online'
            : apiStatus === 'connecting'
              ? 'Connecting...'
              : 'System Offline'
        }}</span>
      </div>
    </div>
  </div>

  <nav class="nav-tabs animate-fade-in delay-100">
    <router-link to="/" class="nav-tab">Dashboard</router-link>
    <router-link to="/orders" class="nav-tab">Pending Orders</router-link>
    <router-link to="/trades" class="nav-tab">Execution History</router-link>
  </nav>

  <main style="margin-top: 2rem">
    <router-view v-slot="{ Component }">
      <transition name="fade-slide" mode="out-in">
        <component :is="Component" />
      </transition>
    </router-view>
  </main>
</template>

<style>
.nav-tabs {
  display: flex;
  gap: 1rem;
  border-bottom: 2px solid var(--border-color);
  padding-bottom: 0px;
}
.nav-tab {
  color: var(--text-secondary);
  text-decoration: none;
  font-weight: 500;
  padding: 0.75rem 1.5rem;
  border-bottom: 2px solid transparent;
  transition: all 0.3s ease;
  margin-bottom: -2px;
}
.nav-tab:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.05);
  border-radius: 6px 6px 0 0;
}
.nav-tab.router-link-active {
  color: var(--text-primary);
  border-bottom: 2px solid var(--accent-primary);
  background: rgba(59, 130, 246, 0.1);
  border-radius: 6px 6px 0 0;
}

.fade-slide-enter-active,
.fade-slide-leave-active {
  transition:
    opacity 0.3s ease,
    transform 0.3s ease;
}
.fade-slide-enter-from {
  opacity: 0;
  transform: translateY(10px);
}
.fade-slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
