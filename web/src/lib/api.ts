// Types mirror the "Dashboard API contract" in docs/ARCHITECTURE.md.

export type User = {
  id: string
  spotify_id: string
  display_name: string | null
  email: string | null
  avatar_url: string | null
  product: string | null // "premium" | "free" | ...
  country: string | null
}

export type LinkStatus = {
  linked: boolean
  status: "ok" | "renewing" | "needs_relink" | null
  scopes: string[]
  /** Non-empty: sign in again to grant them. */
  missing_scopes: string[]
  expires_at: string | null
  last_ok_at: string | null
  last_error: string | null
}

export type AgentSession = {
  id: string
  name: string
  client_id: string
  client_name: string | null
  origin: string | null // e.g. "claude.ai", "chatgpt.com", "localhost"
  enabled: boolean
  read_only: boolean
  allowed_tools: string[] | null // null = all tools
  created_at: string
  last_used_at: string | null
}

export type ToolGroup =
  | "users"
  | "search"
  | "catalog"
  | "library"
  | "follow"
  | "personalization"
  | "playlists"
  | "player"

export type ToolInfo = {
  name: string
  title: string
  description: string
  kind: "read" | "write"
  group: ToolGroup
}

export type ToolCallStatus = "running" | "ok" | "error" | "denied"
export type ToolCall = {
  id: string
  session_id: string
  session_name: string
  tool: string
  /** As logged: long strings are truncated. */
  arguments: Record<string, unknown>
  status: ToolCallStatus
  result: unknown | null
  error: string | null
  duration_ms: number | null
  created_at: string
}

export type OAuthRequest = {
  client_id: string
  client_name: string | null
  client_uri: string | null
  logo_uri: string | null
  origin: string | null
  redirect_uri: string
  scope: string
  /** Set when re-authorizing a client that already has an active session. */
  existing_session: { name: string; read_only: boolean } | null
}

/** What `GET /api/now-playing` returns while something plays; null otherwise. */
export type NowPlaying = {
  is_playing: boolean
  progress: string | null // "m:ss"
  device: {
    name: string
    type: string
    volume_percent: number | null
  } | null
  item: {
    uri: string
    name: string
    artists: { name: string; uri: string }[]
    album: string | null
    image: string | null
    duration: string // "m:ss"
  } | null
}

export type Me = { user: User; link: LinkStatus }

export type ToolCallPage = { items: ToolCall[]; next_before: string | null }

export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = "ApiError"
    this.status = status
    this.code = code
  }
}

export function isUnauthorized(error: unknown): boolean {
  return error instanceof ApiError && error.status === 401
}

type Method = "GET" | "POST" | "PATCH" | "DELETE"

async function request<T>(
  method: Method,
  path: string,
  body?: unknown
): Promise<T> {
  const response = await fetch(path, {
    method,
    credentials: "same-origin",
    // The server rejects non-JSON mutations as a CSRF defence.
    headers:
      method === "GET" ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  })

  if (!response.ok) {
    throw await toApiError(response)
  }
  if (response.status === 204) {
    return undefined as T
  }
  return (await response.json()) as T
}

async function toApiError(response: Response): Promise<ApiError> {
  try {
    const { error, message } = (await response.json()) as {
      error?: string
      message?: string
    }
    return new ApiError(
      response.status,
      error ?? "unknown",
      message ?? response.statusText
    )
  } catch {
    return new ApiError(
      response.status,
      "unknown",
      response.statusText || "Request failed"
    )
  }
}

/** Builds `path?query`, skipping empty values. */
export function withQuery(
  path: string,
  params: Record<string, string | number | null | undefined>
): string {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      search.set(key, String(value))
    }
  }
  const query = search.toString()
  return query ? `${path}?${query}` : path
}

export const api = {
  get: <T>(path: string) => request<T>("GET", path),
  post: <T>(path: string, body: unknown = {}) => request<T>("POST", path, body),
  patch: <T>(path: string, body: unknown) => request<T>("PATCH", path, body),
  delete: (path: string, body?: unknown) => request<void>("DELETE", path, body),
}
