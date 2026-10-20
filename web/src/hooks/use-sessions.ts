import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { api, type AgentSession } from "@/lib/api"

export type SessionPatch = Partial<
  Pick<
    AgentSession,
    | "name"
    | "enabled"
    | "read_only"
    | "use_outbox"
    | "outbox_ttl_minutes"
    | "allowed_tools"
    | "peer_scope"
  >
>

export const sessionsQueryKey = ["sessions"] as const
const sessionQueryKey = (id: string) => [...sessionsQueryKey, id] as const

export function useSessions() {
  return useQuery({
    queryKey: sessionsQueryKey,
    queryFn: () => api.get<AgentSession[]>("/api/sessions"),
  })
}

export function useSession(id: string) {
  return useQuery({
    queryKey: sessionQueryKey(id),
    queryFn: () => api.get<AgentSession>(`/api/sessions/${id}`),
  })
}

/**
 * Applies a patch optimistically. Patches may overlap (several controls), so
 * instead of rolling back to a possibly stale snapshot, the session is
 * refetched once the last in-flight patch settles.
 */
export function useUpdateSession(id: string) {
  const queryClient = useQueryClient()
  const queryKey = sessionQueryKey(id)
  const mutationKey = ["update-session", id]
  return useMutation({
    mutationKey,
    mutationFn: (patch: SessionPatch) =>
      api.patch<AgentSession>(`/api/sessions/${id}`, patch),
    onMutate: async (patch) => {
      await queryClient.cancelQueries({ queryKey })
      queryClient.setQueryData<AgentSession>(
        queryKey,
        (session) => session && { ...session, ...patch }
      )
    },
    onSuccess: () => toast.success("Saved"),
    onError: (error) =>
      toast.error("Couldn't save changes", { description: error.message }),
    onSettled: () => {
      if (queryClient.isMutating({ mutationKey }) === 1) {
        return queryClient.invalidateQueries({ queryKey: sessionsQueryKey })
      }
    },
  })
}

export function useDeleteSession(id: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.delete(`/api/sessions/${id}`),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: sessionQueryKey(id) })
      return queryClient.invalidateQueries({ queryKey: sessionsQueryKey })
    },
  })
}
