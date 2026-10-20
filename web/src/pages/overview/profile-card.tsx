import { TriangleAlertIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { useSignedInMe } from "@/hooks/use-me"
import { initials } from "@/lib/format"

export function ProfileCard() {
  const { user } = useSignedInMe()
  const name = user.display_name ?? user.spotify_id
  const premium = user.product === "premium"
  const details = [user.email, user.country].filter(Boolean).join(" · ")

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
            {user.product && (
              <Badge variant={premium ? "default" : "secondary"}>
                {productLabel(user.product)}
              </Badge>
            )}
          </CardTitle>
          <CardDescription className="truncate">
            {details || user.spotify_id}
          </CardDescription>
        </div>
      </CardHeader>
      {!premium && (
        <CardContent>
          <Alert>
            <TriangleAlertIcon />
            <AlertTitle>Playback control needs Premium</AlertTitle>
            <AlertDescription>
              Spotify only lets apps play, pause, skip or queue on Premium
              accounts. Search, playlists and your library still work.
            </AlertDescription>
          </Alert>
        </CardContent>
      )}
    </Card>
  )
}

/** "premium" → "Premium", "free" → "Free", "open" → "Open". */
function productLabel(product: string): string {
  return product.charAt(0).toUpperCase() + product.slice(1)
}
