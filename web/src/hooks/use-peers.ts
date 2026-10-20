import { keepPreviousData, useQuery } from "@tanstack/react-query"

import { api, withQuery, type PeerSummary } from "@/lib/api"

export function usePeers(query: string) {
  return useQuery({
    queryKey: ["peers", query],
    queryFn: () =>
      api.get<PeerSummary[]>(withQuery("/api/peers", { q: query, limit: 20 })),
    placeholderData: keepPreviousData,
  })
}
