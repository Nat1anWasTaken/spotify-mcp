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
