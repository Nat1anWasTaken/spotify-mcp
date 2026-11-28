import { InfoIcon, TriangleAlertIcon } from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@nat1an/mcp-backbone-dashboard/ui/alert"
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@nat1an/mcp-backbone-dashboard/ui/avatar"
import { Badge } from "@nat1an/mcp-backbone-dashboard/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@nat1an/mcp-backbone-dashboard/ui/card"
import { useSignedInMe } from "@nat1an/mcp-backbone-dashboard"
import { initials } from "@nat1an/mcp-backbone-dashboard"

import type { SpotifyProfile } from "@/lib/types"

export function ProfileCard() {
  const { user } = useSignedInMe()
  const profile = user.profile as SpotifyProfile
  const name = user.name ?? user.subject
  // Usually missing: development-mode apps no longer get product, email or
  // country from Spotify.
  const product = profile.product
  const details = [user.email, profile.country].filter(Boolean).join(" · ")

  return (
    <Card>
      <CardHeader className="flex items-center gap-4">
        <Avatar size="lg">
          {user.avatar_url && <AvatarImage src={user.avatar_url} alt="" />}
          <AvatarFallback>{initials(name)}</AvatarFallback>
        </Avatar>
        <div className="flex min-w-0 flex-col gap-1">
          <CardTitle className="flex flex-wrap items-center gap-2">
            <span className="truncate">{name}</span>
            {product && (
              <Badge variant={product === "premium" ? "default" : "secondary"}>
                {productLabel(product)}
              </Badge>
            )}
          </CardTitle>
          {details && (
            <CardDescription className="truncate">{details}</CardDescription>
          )}
        </div>
      </CardHeader>
      {product !== "premium" && (
        <CardContent>
          {product ? (
            <Alert>
              <TriangleAlertIcon />
              <AlertTitle>Playback control needs Premium</AlertTitle>
              <AlertDescription>
                Spotify only lets apps play, pause, skip or queue on Premium
                accounts. Search, playlists and your library still work.
              </AlertDescription>
            </Alert>
          ) : (
            <p className="flex gap-2 text-sm text-muted-foreground">
              <InfoIcon className="mt-0.5 size-4 shrink-0" />
              Playback control (play, pause, skip, queue) needs Spotify Premium.
            </p>
          )}
        </CardContent>
      )}
    </Card>
  )
}

/** "premium" → "Premium", "free" → "Free", "open" → "Open". */
function productLabel(product: string): string {
  return product.charAt(0).toUpperCase() + product.slice(1)
}
