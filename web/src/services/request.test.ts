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

  it('starts a fresh request when a cancelled request is immediately replaced', async () => {
    const fetchMock = vi.fn((_url: RequestInfo | URL, init?: RequestInit) => {
      if (fetchMock.mock.calls.length === 1) {
        return new Promise((_resolve, reject) => {
          init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')))
        })
      }
      return Promise.resolve({ ok: true, json: async () => ({ value: 2 }) })
    })
    vi.stubGlobal('fetch', fetchMock)

    const controller = new AbortController()
    const first = requestJson<{ value: number }>('/retry', { signal: controller.signal })
    const firstRejection = expect(first).rejects.toMatchObject({ name: 'AbortError' })
    controller.abort()

    const replacement = requestJson<{ value: number }>('/retry')
    await firstRejection
    await expect(replacement).resolves.toEqual({ value: 2 })
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })
})
