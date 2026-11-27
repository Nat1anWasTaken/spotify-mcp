import { useEffect, useState } from "react"
import { AudioLinesIcon, CircleAlertIcon } from "lucide-react"
import { useSearchParams } from "react-router"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@nat1anwastaken/mcp-backbone-dashboard/ui/alert"
import { Button } from "@nat1anwastaken/mcp-backbone-dashboard/ui/button"
import { Spinner } from "@nat1anwastaken/mcp-backbone-dashboard/ui/spinner"
import { nextPath } from "@nat1anwastaken/mcp-backbone-dashboard"

import { signInWithSpotify } from "@/lib/spotify"

/** The `?error=` codes `/auth/spotify/callback` redirects here with. */
const ERRORS: Record<string, { title: string; description: string }> = {
  denied: {
    title: "Sign-in cancelled",
    description:
      "Spotify didn't grant access. Continue with Spotify and choose Agree to use the dashboard.",
  },
  expired: {
    title: "Sign-in expired",
    description:
      "That sign-in attempt timed out or was started in another browser. Please try again.",
  },
  not_allowed: {
    title: "This Spotify account isn't on the list",
    description:
      "This app is in Spotify's development mode, so only the Spotify accounts its owner added under User Management in the Spotify developer dashboard (up to 5) can sign in. Ask the owner to add the email address of your Spotify account, then try again.",
  },
  spotify: {
    title: "Spotify had a problem",
    description:
      "We couldn't complete the sign-in with Spotify. Please try again in a moment.",
  },
}

const UNKNOWN_ERROR = {
  title: "Couldn't sign you in",
  description: "Something went wrong. Please try again.",
}

/** The login page's content: "Continue with Spotify", and why the last
 * attempt failed. */
export function Login() {
  const [searchParams] = useSearchParams()
  const next = nextPath(searchParams)
  const errorCode = searchParams.get("error")
  const error = errorCode ? (ERRORS[errorCode] ?? UNKNOWN_ERROR) : null
  // The browser is leaving for Spotify: keep the button busy until it does.
  const [redirecting, setRedirecting] = useState(false)

  // Coming back from Spotify with the Back button restores this page from
  // the back/forward cache, still busy: make the button usable again.
  useEffect(() => {
    const reset = (event: PageTransitionEvent) =>
      event.persisted && setRedirecting(false)
    window.addEventListener("pageshow", reset)
    return () => window.removeEventListener("pageshow", reset)
  }, [])

  function continueWithSpotify() {
    setRedirecting(true)
    signInWithSpotify(next)
  }

  return (
    <>
      {error && (
        <Alert variant="destructive">
          <CircleAlertIcon />
          <AlertTitle>{error.title}</AlertTitle>
          <AlertDescription>{error.description}</AlertDescription>
        </Alert>
      )}
      <Button size="lg" disabled={redirecting} onClick={continueWithSpotify}>
        {redirecting ? <Spinner /> : <AudioLinesIcon />}
        Continue with Spotify
      </Button>
    </>
  )
}
