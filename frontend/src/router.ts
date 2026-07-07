import { createRouter, createWebHistory } from 'vue-router';
import DashboardView from './views/DashboardView.vue';
import OrdersView from './views/OrdersView.vue';
import TradesView from './views/TradesView.vue';

const routes = [
  { path: '/', component: DashboardView },
  { path: '/orders', component: OrdersView },
  { path: '/trades', component: TradesView },
];

export const router = createRouter({
  history: createWebHistory(),
  routes,
});
