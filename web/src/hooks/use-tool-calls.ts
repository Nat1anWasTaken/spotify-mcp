import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import { toast } from "sonner"

import {
  api,
  withQuery,
  type ToolCall,
  type ToolCallDetail,
  type ToolCallPage,
  type ToolCallStatus,
} from "@/lib/api"
import { sessionsQueryKey } from "@/hooks/use-sessions"
import { closeNotification } from "@/lib/push"

export type ToolCallFilters = {
  session_id?: string
  status?: ToolCallStatus
}

const toolCallsQueryKey = ["tool-calls"] as const

function fetchToolCalls(
  params: ToolCallFilters & { before?: string; limit?: number }
) {
  return api.get<ToolCallPage>(withQuery("/api/tool-calls", params))
}

function isUnfinished(call: ToolCall): boolean {
  return call.status === "running" || call.status === "queued"
}

export function useToolCallLog(filters: ToolCallFilters) {
  return useInfiniteQuery({
    queryKey: [...toolCallsQueryKey, "log", filters],
    queryFn: ({ pageParam }) =>
      fetchToolCalls({ ...filters, before: pageParam, limit: 50 }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_before ?? undefined,
    // Keep polling while any loaded call can still change status.
    refetchInterval: (query) =>
      query.state.data?.pages.some((page) => page.items.some(isUnfinished))
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

export function useOutbox() {
  return useQuery({
    queryKey: [...toolCallsQueryKey, "outbox"],
    queryFn: () => fetchToolCalls({ status: "queued", limit: 100 }),
    select: (page) => page.items,
    refetchInterval: 3000,
  })
}

/** One call, polled while it can still change status. */
export function useToolCall(id: string) {
  return useQuery({
    queryKey: [...toolCallsQueryKey, "detail", id],
    queryFn: () => api.get<ToolCallDetail>(`/api/tool-calls/${id}`),
    refetchInterval: (query) =>
      query.state.data && isUnfinished(query.state.data) ? 3000 : false,
  })
}

export type Decision = {
  id: string
  /** Arguments the user changed, merged over the agent's when approving. */
  arguments?: Record<string, unknown>
}

export function useDecideToolCall(decision: "approve" | "discard") {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, arguments: edits }: Decision) =>
      api.post<ToolCall>(
        `/api/tool-calls/${id}/${decision}`,
        edits ? { arguments: edits } : {}
      ),
    onSuccess: (call) => {
      void closeNotification(call.id.replace(/-/g, ""))
      if (decision === "approve") {
        toastApproval(call)
      } else {
        toast.success(`Discarded ${call.tool}`)
      }
    },
    // e.g. 409 when the call already expired; the refetch below drops it.
    onError: (error) => toast.error(error.message),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: toolCallsQueryKey }),
        queryClient.invalidateQueries({ queryKey: sessionsQueryKey }),
      ]),
  })
}

/** Approved calls are re-checked against the session's current settings, so
 * approving doesn't guarantee the call ran. */
function toastApproval(call: ToolCall) {
  switch (call.status) {
    case "ok":
      toast.success(`Approved ${call.tool}`)
      break
    case "denied":
      toast.error(`${call.tool} was denied`, {
        description: call.error ?? undefined,
      })
      break
    case "error":
      toast.error(`${call.tool} failed`, {
        description: call.error ?? undefined,
      })
      break
    default:
      toast(`Approved ${call.tool}`, { description: "Still running." })
  }
}
