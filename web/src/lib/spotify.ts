/** Where "Continue with Spotify" and "Re-link Spotify" send the browser. */
export function spotifySignInUrl(next: string): string {
  return `/auth/spotify/start?next=${encodeURIComponent(next)}`
}

/**
 * A full-page navigation to Spotify's consent screen; the server redirects
 * back to `next` (a same-origin path) afterwards.
 */
export function signInWithSpotify(next: string) {
  window.location.assign(spotifySignInUrl(next))
}

const DAY_MS = 24 * 60 * 60 * 1000

/** How early the dashboard starts nagging before the refresh token expires. */
export const REAUTHORIZE_WARNING_DAYS = 14

/**
 * Spotify refresh tokens expire six months after sign-in, whatever their use:
 * "expired" once `reauthorize_by` has passed, "soon" within the warning window.
 */
export function reauthorizeState(
  reauthorizeBy: string | null,
  now: number
): "ok" | "soon" | "expired" {
  if (reauthorizeBy === null) {
    return "ok"
  }
  const remaining = new Date(reauthorizeBy).getTime() - now
  if (remaining <= 0) {
    return "expired"
  }
  return remaining <= REAUTHORIZE_WARNING_DAYS * DAY_MS ? "soon" : "ok"
}
