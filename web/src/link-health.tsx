import type { LinkHealth, Me } from "@nat1an/mcp-backbone-dashboard"
import { Button } from "@nat1an/mcp-backbone-dashboard/ui/button"
import { formatDate } from "@nat1an/mcp-backbone-dashboard"

import { reauthorizeState, signInWithSpotify } from "@/lib/spotify"
import type { LinkStatus } from "@/lib/types"

function relink(error: boolean) {
  return (
    <Button
      size="sm"
      variant={error ? "destructive" : "default"}
      onClick={() => signInWithSpotify("/")}
    >
      Re-link
    </Button>
  )
}

/**
 * Whether the Spotify link works: broken, expired (refresh tokens last six
 * months), lacking newly required scopes, or about to expire.
 */
export function spotifyLinkHealth(me: Me<unknown>): LinkHealth {
  const link = me.link as LinkStatus
  const reauthorize = reauthorizeState(link.reauthorize_by, Date.now())
  if (!link.linked || link.status === "needs_relink") {
    return {
      ok: false,
      title: "Your Spotify link has stopped working",
      description:
        "Agents can't reach Spotify until you sign in with Spotify again.",
      action: relink(true),
    }
  }
  if (reauthorize === "expired") {
    return {
      ok: false,
      title: "Your Spotify sign-in has expired",
      description:
        "Spotify ends app access six months after you sign in. Sign in with Spotify again so your agents keep working.",
      action: relink(true),
    }
  }
  if (link.missing_scopes.length > 0) {
    return {
      ok: false,
      title: "Spotify needs a few more permissions",
      description:
        "Agents can't use Spotify until you sign in with Spotify again and grant the new permissions.",
      action: relink(true),
    }
  }
  if (reauthorize === "soon" && link.reauthorize_by) {
    return {
      ok: true,
      title: `Sign in with Spotify again by ${formatDate(link.reauthorize_by)}`,
      description:
        "Spotify ends app access six months after you sign in. Signing in again now resets the clock.",
      action: relink(false),
    }
  }
  return { ok: true }
}
