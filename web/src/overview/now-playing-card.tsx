import {
  DiscIcon,
  MonitorSpeakerIcon,
  MusicIcon,
  PauseIcon,
  PlayIcon,
} from "lucide-react"

import { ErrorAlert } from "@nat1anwastaken/mcp-backbone-dashboard"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@nat1anwastaken/mcp-backbone-dashboard/ui/card"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@nat1anwastaken/mcp-backbone-dashboard/ui/empty"
import { Progress } from "@nat1anwastaken/mcp-backbone-dashboard/ui/progress"
import { Skeleton } from "@nat1anwastaken/mcp-backbone-dashboard/ui/skeleton"
import { useNowPlaying } from "@/hooks/use-now-playing"
import type { NowPlaying } from "@/lib/types"

export function NowPlayingCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Now playing</CardTitle>
      </CardHeader>
      <CardContent>
        <NowPlayingContent />
      </CardContent>
    </Card>
  )
}

function NowPlayingContent() {
  const nowPlaying = useNowPlaying()

  if (nowPlaying.isPending) {
    return <Skeleton className="h-20 w-full" />
  }
  // Keep showing the last state if a background refetch fails.
  if (nowPlaying.isError && nowPlaying.data === undefined) {
    return (
      <ErrorAlert title="Couldn't reach Spotify" error={nowPlaying.error} />
    )
  }
  const playing = nowPlaying.data
  if (!playing?.item) {
    return <NothingPlaying device={playing?.device ?? null} />
  }
  return <Track playing={playing} item={playing.item} />
}

type TrackProps = {
  playing: NowPlaying
  item: NonNullable<NowPlaying["item"]>
}

function Track({ playing, item }: TrackProps) {
  const progress = playing.progress_ms
  const duration = item.duration_ms
  const percent =
    progress !== null && duration
      ? Math.min(100, (progress / duration) * 100)
      : null

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-4">
        {item.image ? (
          <img
            src={item.image}
            alt=""
            className="size-16 shrink-0 rounded-md object-cover"
          />
        ) : (
          <div className="flex size-16 shrink-0 items-center justify-center rounded-md bg-muted">
            <DiscIcon className="text-muted-foreground" />
          </div>
        )}
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="truncate font-medium" title={item.name}>
            {item.name}
          </span>
          <CardDescription className="truncate">
            {item.artists.map((artist) => artist.name).join(", ")}
            {item.album && ` · ${item.album}`}
          </CardDescription>
        </div>
      </div>
      {percent !== null && (
        <Progress value={percent} aria-label="Playback progress" />
      )}
      <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
        <span className="flex items-center gap-1">
          {playing.is_playing ? (
            <PlayIcon className="size-3" aria-label="Playing" />
          ) : (
            <PauseIcon className="size-3" aria-label="Paused" />
          )}
          <span className="tabular-nums">
            {playing.progress ?? "–:––"} / {item.duration}
          </span>
        </span>
        {playing.device && <Device device={playing.device} />}
      </div>
    </div>
  )
}

function Device({ device }: { device: NonNullable<NowPlaying["device"]> }) {
  return (
    <span className="flex min-w-0 items-center gap-1">
      <MonitorSpeakerIcon className="size-3 shrink-0" />
      <span className="truncate">
        {device.name}
        {device.volume_percent !== null && ` · ${device.volume_percent}%`}
      </span>
    </span>
  )
}

function NothingPlaying({ device }: { device: NowPlaying["device"] }) {
  return (
    <Empty className="p-4">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MusicIcon />
        </EmptyMedia>
        <EmptyTitle>Nothing playing</EmptyTitle>
        <EmptyDescription>
          {device
            ? `${device.name} is idle.`
            : "Start playing on any Spotify app and it shows up here."}
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}
