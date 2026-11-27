import { useQuery } from "@tanstack/react-query"

import { api } from "@nat1anwastaken/mcp-backbone-dashboard"

import type { NowPlaying } from "@/lib/types"

/**
 * What's playing on the user's Spotify account (null when nothing is), polled
 * every 10 s. TanStack Query pauses the polling while the tab is hidden and
 * refetches when it becomes visible again.
 */
export function useNowPlaying() {
  return useQuery({
    queryKey: ["now-playing"],
    queryFn: () => api.get<NowPlaying | null>("/api/now-playing"),
    refetchInterval: 10_000,
    refetchIntervalInBackground: false,
  })
}
