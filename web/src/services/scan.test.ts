import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchScanJob } from './scan'

describe('scan service', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('returns the scan job contract', async () => {
    const job = { job_id: 7, status: 'running', progress: 25 }
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => job }))

    await expect(fetchScanJob(7)).resolves.toEqual(job)
    expect(fetch).toHaveBeenCalledWith('/api/v1/scan-jobs/7')
  })

  it('normalizes API errors', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      json: async () => ({ error: 'Job not found' }),
    }))

    await expect(fetchScanJob(9)).rejects.toThrow('Job not found')
  })
})
