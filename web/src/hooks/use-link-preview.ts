import { useQuery } from "@tanstack/react-query"

import { api, withQuery, type LinkPreview } from "@/lib/api"

export function useLinkPreview(url: string) {
  return useQuery({
    queryKey: ["link-preview", url],
    queryFn: () =>
      api.get<LinkPreview>(withQuery("/api/link-preview", { url })),
    // The server caches previews for as long.
    staleTime: 10 * 60 * 1000,
    retry: false,
  })
}
