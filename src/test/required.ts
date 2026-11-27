import { assert } from 'vitest';

export function required<T>(value: T): NonNullable<T> {
  assert(value != null, 'Expected the test value to be present');
  return value;
}
