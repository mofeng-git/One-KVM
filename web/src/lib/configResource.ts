import { ref, type Ref } from 'vue'

/** Shared request state for a service with several independently cached endpoints. */
export function createRequestState() {
  const loading = ref(false)
  const error = ref<string | null>(null)
  let pending = 0

  async function run<T>(request: () => Promise<T>): Promise<T> {
    pending++
    loading.value = true
    error.value = null
    try {
      return await request()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message
        : typeof cause === 'string' ? cause : 'Unknown error'
      throw cause
    } finally {
      loading.value = --pending > 0
    }
  }

  return { loading, error, run }
}

/** Cache successful reads and coalesce concurrent refreshes of the same endpoint. */
export function createConfigResource<T>(
  fetcher: () => Promise<T>,
  state = createRequestState(),
  onLoaded?: (response: T) => void,
) {
  const data = ref(null) as Ref<T | null>
  let inFlight: Promise<T> | null = null

  function refresh(): Promise<T> {
    if (inFlight) return inFlight
    inFlight = state.run(async () => {
      const response = await fetcher()
      data.value = response
      onLoaded?.(response)
      return response
    }).finally(() => {
      inFlight = null
    })
    return inFlight
  }

  function ensure(): Promise<T> {
    return data.value === null ? refresh() : Promise.resolve(data.value)
  }

  return { data, loading: state.loading, error: state.error, refresh, ensure }
}
