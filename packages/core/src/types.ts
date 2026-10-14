/** Timestamps are Unix milliseconds. Secrets and account identity never enter this contract. */
export interface QuotaMeter {
  kind: 'quota'
  id: string
  label: string
  remainingPercent: number
  durationMinutes: number
  resetsAt: number | null
}

export interface BalanceMeter {
  kind: 'balance'
  id: string
  label: string
  /** Decimal string: do not round currency before displaying it. */
  remaining: string
  currency: string
}

export interface UsageSnapshot {
  provider: string
  plan: string | null
  observedAt: number
  meters: (QuotaMeter | BalanceMeter)[]
}

export interface UsageError {
  code: 'cli_missing' | 'not_logged_in' | 'api_key_login' | 'timeout' | 'protocol' | 'query_failed' | 'busy'
  message: string
}

export interface UsageProvider {
  readonly id: string
  read: () => Promise<UsageSnapshot>
}
