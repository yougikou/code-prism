import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clearRequestCache, requestJson } from './request'

describe('requestJson', () => {
  beforeEach(clearRequestCache)
  afterEach(() => vi.unstubAllGlobals())

  it('deduplicates concurrent requests and caches successful responses', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ value: 1 }) })
    vi.stubGlobal('fetch', fetchMock)
    const [first, second] = await Promise.all([
      requestJson<{ value: number }>('/same', { cacheTtlMs: 1000 }),
      requestJson<{ value: number }>('/same', { cacheTtlMs: 1000 }),
    ])
    expect(first).toEqual({ value: 1 })
    expect(second).toEqual({ value: 1 })
    await requestJson('/same', { cacheTtlMs: 1000 })
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  it('aborts the underlying request when all subscribers cancel', async () => {
    vi.stubGlobal('fetch', vi.fn((_url: RequestInfo | URL, init?: RequestInit) => new Promise((_resolve, reject) => {
      init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')))
    })))
    const controller = new AbortController()
    const result = requestJson('/slow', { signal: controller.signal })
    controller.abort()
    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
  })
})
