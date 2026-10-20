import { useEffect, useState } from "react"
import { AudioLinesIcon, CircleAlertIcon } from "lucide-react"
import { useSearchParams } from "react-router"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
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

/** Only same-origin paths, so `?next=` can't send the user elsewhere. */
function safeNext(value: string | null): string {
  return value?.startsWith("/") && !value.startsWith("//") ? value : "/"
}

export function LoginPage() {
  const [searchParams] = useSearchParams()
  const next = safeNext(searchParams.get("next"))
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
    <div className="flex min-h-svh items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <img src="/spotify.png" alt="" className="size-8" />
            Spotify MCP
          </CardTitle>
          <CardDescription>
            Let AI agents search, manage your playlists and library, and control
            playback on your Spotify account, with permissions you choose.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {error && (
            <Alert variant="destructive">
              <CircleAlertIcon />
              <AlertTitle>{error.title}</AlertTitle>
              <AlertDescription>{error.description}</AlertDescription>
            </Alert>
          )}
          <Button
            size="lg"
            disabled={redirecting}
            onClick={continueWithSpotify}
          >
            {redirecting ? <Spinner /> : <AudioLinesIcon />}
            Continue with Spotify
          </Button>
        </CardContent>
        <CardFooter>
          <p className="text-xs text-muted-foreground">
            Signing in also links your Spotify account. Your Spotify tokens are
            stored encrypted, and you can disconnect any agent at any time.
          </p>
        </CardFooter>
      </Card>
    </div>
  )
}
