/**
 * Unit tests for the formatTime utility extracted from App.vue.
 * Tests edge cases: zero, valid timestamp, string input, missing value.
 */
import { describe, it, expect } from 'vitest';

// Replicated from App.vue — extract to utils if needed
const formatTime = (ts: number | string): string => {
  if (!ts) return '-';
  const date = new Date(ts);
  return date.toLocaleTimeString(undefined, {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

describe('formatTime', () => {
  it('returns "-" for zero value', () => {
    expect(formatTime(0)).toBe('-');
  });

  it('returns "-" for empty string', () => {
    expect(formatTime('')).toBe('-');
  });

  it('formats a valid Unix timestamp (ms) as HH:MM:SS', () => {
    // 2024-01-01T12:00:00.000Z in UTC is 13:00:00 at UTC+1, but we test shape
    const ts = new Date('2024-01-01T00:00:00.000Z').getTime();
    const result = formatTime(ts);
    // Should match the HH:MM:SS pattern (locale-independent check)
    expect(result).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('formats a string ISO date correctly', () => {
    const result = formatTime('2024-06-15T10:30:00.000Z');
    expect(result).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('returns a non-dash value for a valid recent timestamp', () => {
    const ts = Date.now();
    const result = formatTime(ts);
    expect(result).not.toBe('-');
    expect(result.length).toBeGreaterThan(0);
  });
});
