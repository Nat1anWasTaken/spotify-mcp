import { useInfiniteQuery, useQuery } from "@tanstack/react-query"

import {
  api,
  withQuery,
  type ToolCall,
  type ToolCallPage,
  type ToolCallStatus,
} from "@/lib/api"

export type ToolCallFilters = {
  session_id?: string
  status?: ToolCallStatus
  tool?: string
}

const toolCallsQueryKey = ["tool-calls"] as const

function fetchToolCalls(
  params: ToolCallFilters & { before?: string; limit?: number }
) {
  return api.get<ToolCallPage>(withQuery("/api/tool-calls", params))
}

function isRunning(call: ToolCall): boolean {
  return call.status === "running"
}

export function useToolCallLog(filters: ToolCallFilters) {
  return useInfiniteQuery({
    queryKey: [...toolCallsQueryKey, "log", filters],
    queryFn: ({ pageParam }) =>
      fetchToolCalls({ ...filters, before: pageParam, limit: 50 }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_before ?? undefined,
    // Keep polling while any loaded call is still running.
    refetchInterval: (query) =>
      query.state.data?.pages.some((page) => page.items.some(isRunning))
        ? 3000
        : false,
  })
}

export function useRecentToolCalls(sessionId: string) {
  return useQuery({
    queryKey: [...toolCallsQueryKey, "recent", sessionId],
    queryFn: () => fetchToolCalls({ session_id: sessionId, limit: 10 }),
    select: (page) => page.items,
  })
}
