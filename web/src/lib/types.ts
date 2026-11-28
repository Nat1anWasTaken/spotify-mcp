// Shapes of the server's Spotify-specific API.

export type LinkStatus = {
  linked: boolean
  status: "ok" | "renewing" | "needs_relink" | null
  scopes: string[]
  /** Non-empty: sign in again to grant them. */
  missing_scopes: string[]
  expires_at: string | null
  last_ok_at: string | null
  last_error: string | null
  /** The last Spotify sign-in. */
  authorized_at: string | null
  /** `authorized_at` + 6 months: the refresh token's hard expiry. */
  reauthorize_by: string | null
}
export type NowPlaying = {
  is_playing: boolean
  progress: string | null // "m:ss"
  progress_ms: number | null
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
    duration_ms: number
  } | null
}

/** What the profile keeps besides the framework's fields. */
export type SpotifyProfile = { product?: string; country?: string }
