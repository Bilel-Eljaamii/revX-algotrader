/**
 * Vitest setup – applies before every test file.
 * Provides a minimal localStorage stub for the happy-dom environment
 * (some vitest versions don't wire it up automatically).
 */
import { vi } from 'vitest';

const localStorageStore: Record<string, string> = {};

Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  writable: true,
  value: {
    getItem: vi.fn((key: string) => localStorageStore[key] ?? null),
    setItem: vi.fn((key: string, val: string) => {
      localStorageStore[key] = val;
    }),
    removeItem: vi.fn((key: string) => {
      delete localStorageStore[key];
    }),
    clear: vi.fn(() => {
      Object.keys(localStorageStore).forEach((k) => delete localStorageStore[k]);
    }),
    length: 0,
    key: vi.fn((_index: number) => null),
  },
});
