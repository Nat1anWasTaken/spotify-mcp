import { CircleAlertIcon, TriangleAlertIcon } from "lucide-react"

import {
  Alert,
  AlertAction,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { useSignedInMe } from "@/hooks/use-me"
import { signInWithSpotify } from "@/lib/spotify"

/**
 * Shown on every dashboard page while the Spotify link is broken, or when it
 * lacks scopes that a newer version of the server asks for.
 */
export function RelinkBanner() {
  const { link } = useSignedInMe()
  const broken = !link.linked || link.status === "needs_relink"
  if (!broken && link.missing_scopes.length === 0) {
    return null
  }

  return (
    <Alert variant={broken ? "destructive" : "default"}>
      {broken ? <CircleAlertIcon /> : <TriangleAlertIcon />}
      <AlertTitle>
        {broken
          ? "Your Spotify link has stopped working"
          : "Spotify needs a few more permissions"}
      </AlertTitle>
      <AlertDescription>
        {broken
          ? "Agents can't reach Spotify until you sign in with Spotify again."
          : "Some tools won't work until you sign in with Spotify again and grant the new permissions."}
      </AlertDescription>
      <AlertAction>
        <Button
          size="sm"
          variant={broken ? "destructive" : "default"}
          onClick={() => signInWithSpotify("/")}
        >
          Re-link
        </Button>
      </AlertAction>
    </Alert>
  )
}
