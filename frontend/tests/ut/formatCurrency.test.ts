/**
 * Unit tests for the formatCurrency utility extracted from App.vue.
 */
import { describe, it, expect } from 'vitest';

const formatCurrency = (val: number | string): number | string => {
  const num = Number(val);
  if (isNaN(num)) return val;
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  }).format(num);
};

describe('formatCurrency', () => {
  it('formats a number as USD currency', () => {
    const result = formatCurrency(1234.5);
    expect(result).toBe('$1,234.50');
  });

  it('formats zero as $0.00', () => {
    expect(formatCurrency(0)).toBe('$0.00');
  });

  it('formats a numeric string correctly', () => {
    expect(formatCurrency('99.99')).toBe('$99.99');
  });

  it('returns original value when input is non-numeric string', () => {
    expect(formatCurrency('invalid')).toBe('invalid');
  });

  it('formats negative numbers correctly', () => {
    const result = formatCurrency(-50.25);
    expect(result).toContain('50.25');
  });

  it('preserves up to 6 decimal places', () => {
    expect(formatCurrency(1.000185)).toBe('$1.000185');
    expect(formatCurrency(1.9999999)).toBe('$2.00');
  });

  it('handles large numbers with comma separators', () => {
    const result = formatCurrency(1000000);
    expect(result).toBe('$1,000,000.00');
  });
});
