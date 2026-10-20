// Types mirror the "Dashboard API contract" in docs/ARCHITECTURE.md.

export type User = {
  id: string
  github_login: string
  name: string | null
  avatar_url: string | null
}

export type TelegramStatus = {
  linked: boolean
  account: {
    tg_user_id: number
    display_name: string
    username: string | null
    phone: string | null
  } | null
}

// QR login flow, polled by the dashboard about once per second.
export type LoginState =
  | { state: "idle" }
  | { state: "waiting_scan"; qr_url: string; expires_at: string } // render qr_url as a QR code
  | { state: "password_required"; password_hint: string | null } // 2FA
  | { state: "success"; account: TelegramStatus["account"] }
  | { state: "failed"; error: string }

export type PeerKind = "user" | "bot" | "group" | "channel"
export type PeerSummary = {
  id: number
  kind: PeerKind
  title: string
  username: string | null
}

export type PeerScope = { mode: "all" | "allow" | "deny"; peers: PeerSummary[] }

export type AgentSession = {
  id: string
  name: string
  client_id: string
  client_name: string | null
  origin: string | null // e.g. "claude.ai", "chatgpt.com", "localhost"
  enabled: boolean
  read_only: boolean
  use_outbox: boolean
  /** How long a write call waits in the outbox: 1 to 10080 (a week). */
  outbox_ttl_minutes: number
  allowed_tools: string[] | null // null = all tools
  peer_scope: PeerScope
  created_at: string
  last_used_at: string | null
  outbox_count: number
}

export type ToolInfo = {
  name: string
  title: string
  description: string
  kind: "read" | "write"
}

export type ToolCallStatus =
  "running" | "ok" | "error" | "denied" | "queued" | "discarded" | "expired"
export type ToolCall = {
  id: string
  session_id: string
  session_name: string
  tool: string
  /** As logged: long strings are truncated. */
  arguments: Record<string, unknown>
  /** The agent's arguments, when the user edited them before approving. */
  original_arguments: Record<string, unknown> | null
  status: ToolCallStatus
  result: unknown | null
  error: string | null
  duration_ms: number | null
  created_at: string
  outbox_expires_at: string | null
  decided_at: string | null
}

/** A call as its outbox page shows it. */
export type ToolCallDetail = ToolCall & {
  /** The full arguments while the call is in the outbox; null afterwards. */
  editable_arguments: Record<string, unknown> | null
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
  existing_session: {
    name: string
    read_only: boolean
    use_outbox: boolean
  } | null
}

export type ParseMode = "markdown" | "html" | "plain"

/** Offsets and lengths count UTF-16 code units, like JavaScript strings. */
export type TextEntity = { offset: number; length: number } & (
  | {
      type:
        | "bold"
        | "italic"
        | "underline"
        | "strike"
        | "spoiler"
        | "code"
        | "url"
        | "blockquote"
        | "mention"
    }
  | { type: "pre"; language: string }
  | { type: "text_url"; url: string }
)

/** Message text as Telegram will receive it. */
export type FormattedText = { text: string; entities: TextEntity[] }

/** Every field is null when the link couldn't be previewed. */
export type LinkPreview = {
  final_url: string | null
  title: string | null
  description: string | null
  image: string | null
  site_name: string | null
}

export type Me = { user: User; telegram: TelegramStatus }

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
