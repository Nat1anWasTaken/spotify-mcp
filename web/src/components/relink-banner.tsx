import { CircleAlertIcon, TriangleAlertIcon } from "lucide-react"

import {
  Alert,
  AlertAction,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { useSignedInMe } from "@/hooks/use-me"
import { useNow } from "@/hooks/use-now"
import { formatDate } from "@/lib/format"
import { reauthorizeState, signInWithSpotify } from "@/lib/spotify"

type Notice = { error: boolean; title: string; description: string }

/**
 * Shown on every dashboard page while the Spotify link is broken, about to
 * expire (refresh tokens last six months), or lacking newly required scopes.
 */
export function RelinkBanner() {
  const { link } = useSignedInMe()
  const now = useNow(60_000)
  const reauthorize = reauthorizeState(link.reauthorize_by, now)

  let notice: Notice | null = null
  if (!link.linked || link.status === "needs_relink") {
    notice = {
      error: true,
      title: "Your Spotify link has stopped working",
      description:
        "Agents can't reach Spotify until you sign in with Spotify again.",
    }
  } else if (reauthorize === "expired") {
    notice = {
      error: true,
      title: "Your Spotify sign-in has expired",
      description:
        "Spotify ends app access six months after you sign in. Sign in with Spotify again so your agents keep working.",
    }
  } else if (reauthorize === "soon" && link.reauthorize_by) {
    notice = {
      error: false,
      title: `Sign in with Spotify again by ${formatDate(link.reauthorize_by)}`,
      description:
        "Spotify ends app access six months after you sign in. Signing in again now resets the clock.",
    }
  } else if (link.missing_scopes.length > 0) {
    notice = {
      error: true,
      title: "Spotify needs a few more permissions",
      description:
        "Agents can't use Spotify until you sign in with Spotify again and grant the new permissions.",
    }
  }
  if (!notice) {
    return null
  }

  return (
    <Alert variant={notice.error ? "destructive" : "default"}>
      {notice.error ? <CircleAlertIcon /> : <TriangleAlertIcon />}
      <AlertTitle>{notice.title}</AlertTitle>
      <AlertDescription>{notice.description}</AlertDescription>
      <AlertAction>
        <Button
          size="sm"
          variant={notice.error ? "destructive" : "default"}
          onClick={() => signInWithSpotify("/")}
        >
          Re-link
        </Button>
      </AlertAction>
    </Alert>
  )
}
