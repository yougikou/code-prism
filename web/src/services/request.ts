interface CacheEntry {
  expiresAt: number
  value: unknown
}

interface PendingRequest {
  controller: AbortController
  promise: Promise<unknown>
  subscribers: number
}

const cache = new Map<string, CacheEntry>()
const pending = new Map<string, PendingRequest>()

export interface RequestOptions {
  signal?: AbortSignal
  cacheTtlMs?: number
}

export function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

export function clearRequestCache(): void {
  cache.clear()
}

export async function requestJson<T>(url: string, options: RequestOptions = {}): Promise<T> {
  const now = Date.now()
  const cached = cache.get(url)
  if (cached && cached.expiresAt > now) return cached.value as T
  if (cached) cache.delete(url)

  let current = pending.get(url)
  if (current?.controller.signal.aborted) {
    pending.delete(url)
    current = undefined
  }
  if (!current) {
    const controller = new AbortController()
    const promise = fetch(url, { signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`.trim())
      const value = await response.json() as T
      if ((options.cacheTtlMs ?? 0) > 0) {
        cache.set(url, { value, expiresAt: Date.now() + options.cacheTtlMs! })
      }
      return value
    }).finally(() => {
      if (pending.get(url)?.controller === controller) pending.delete(url)
    })
    current = { controller, promise, subscribers: 0 }
    pending.set(url, current)
  }

  current.subscribers += 1
  const request = current
  return new Promise<T>((resolve, reject) => {
    let active = true
    const release = () => {
      if (!active) return
      active = false
      request.subscribers -= 1
      options.signal?.removeEventListener('abort', onAbort)
    }
    const onAbort = () => {
      release()
      if (request.subscribers === 0 && pending.get(url) === request) {
        pending.delete(url)
        request.controller.abort()
      }
      reject(new DOMException('Request aborted', 'AbortError'))
    }
    if (options.signal?.aborted) return onAbort()
    options.signal?.addEventListener('abort', onAbort, { once: true })
    request.promise.then(
      value => { if (active) { release(); resolve(value as T) } },
      error => { if (active) { release(); reject(error) } },
    )
  })
}
