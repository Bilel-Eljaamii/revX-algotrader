import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  server: {
    proxy: {
      '/proxy': {
        target: 'http://127.0.0.1:30091',
        changeOrigin: true,
      },
      '/health': {
        target: 'http://127.0.0.1:30091',
        changeOrigin: true,
      },
    },
  },
});
