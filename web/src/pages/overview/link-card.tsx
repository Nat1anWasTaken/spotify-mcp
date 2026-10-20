import type { ReactNode } from "react"
import { TriangleAlertIcon } from "lucide-react"

import { LinkStatusBadge } from "@/components/link-status-badge"
import { RelativeTime } from "@/components/relative-time"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { useSignedInMe } from "@/hooks/use-me"
import { useNow } from "@/hooks/use-now"
import { formatDate } from "@/lib/format"
import { reauthorizeState, signInWithSpotify } from "@/lib/spotify"

export function LinkCard() {
  const { link } = useSignedInMe()
  const reauthorize = reauthorizeState(link.reauthorize_by, useNow(60_000))
  const needsRelink =
    !link.linked ||
    link.status === "needs_relink" ||
    reauthorize !== "ok" ||
    link.missing_scopes.length > 0

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          Spotify link
          <LinkStatusBadge link={link} />
        </CardTitle>
        <CardDescription>
          The Spotify access your agents use. It renews itself as needed, but
          Spotify ends it six months after each sign-in.
        </CardDescription>
        <CardAction>
          <Button
            variant={needsRelink ? "default" : "outline"}
            size="sm"
            onClick={() => signInWithSpotify("/")}
          >
            Re-link Spotify
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {link.missing_scopes.length > 0 && (
          <Alert>
            <TriangleAlertIcon />
            <AlertTitle>Some permissions are missing</AlertTitle>
            <AlertDescription>
              <p>
                Re-link Spotify to grant them; agents can&apos;t use Spotify
                until you do.
              </p>
              <ScopeList scopes={link.missing_scopes} />
            </AlertDescription>
          </Alert>
        )}
        <dl className="grid gap-x-6 gap-y-3 text-sm sm:grid-cols-[auto_1fr]">
          {link.reauthorize_by && (
            <Row label="Sign in again by">
              <span
                className={
                  reauthorize === "ok"
                    ? undefined
                    : reauthorize === "expired"
                      ? "font-medium text-destructive"
                      : "font-medium"
                }
              >
                {formatDate(link.reauthorize_by)} (
                <RelativeTime value={link.reauthorize_by} />)
              </span>
            </Row>
          )}
          {link.authorized_at && (
            <Row label="Signed in with Spotify">
              <RelativeTime value={link.authorized_at} />
            </Row>
          )}
          <Row label="Last successful call">
            {link.last_ok_at ? (
              <RelativeTime value={link.last_ok_at} />
            ) : (
              "Never"
            )}
          </Row>
          <Row label="Access token expires">
            {link.expires_at ? (
              <RelativeTime value={link.expires_at} />
            ) : (
              "Unknown"
            )}
          </Row>
          {link.last_error && (
            <Row label="Last error">
              <span className="text-destructive">{link.last_error}</span>
            </Row>
          )}
          <Row label="Granted scopes">
            {link.scopes.length > 0 ? (
              <ScopeList scopes={link.scopes} />
            ) : (
              "None"
            )}
          </Row>
        </dl>
      </CardContent>
    </Card>
  )
}

function ScopeList({ scopes }: { scopes: string[] }) {
  return (
    <ul className="flex flex-wrap gap-1">
      {scopes.map((scope) => (
        <li key={scope}>
          <Badge variant="outline" className="font-mono">
            {scope}
          </Badge>
        </li>
      ))}
    </ul>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd>{children}</dd>
    </>
  )
}
