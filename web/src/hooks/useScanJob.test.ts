import { describe, expect, it } from 'vitest'
import { nextPollingDelay } from './useScanJob'

describe('nextPollingDelay', () => {
  it('resets after progress and backs off while progress is unchanged', () => {
    expect(nextPollingDelay(8_000, true)).toBe(2_000)
    expect(nextPollingDelay(2_000, false)).toBe(3_000)
    expect(nextPollingDelay(12_000, false)).toBe(15_000)
  })
})
