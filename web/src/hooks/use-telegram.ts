import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { api, type LoginState } from "@/lib/api"
import { meQueryKey } from "@/hooks/use-me"

const loginQueryKey = ["telegram-login"]
const LOGIN_PATH = "/api/telegram/login"

function isInProgress(state: LoginState | undefined): boolean {
  return state?.state === "waiting_scan" || state?.state === "password_required"
}

export function useTelegramLogin() {
  return useQuery({
    queryKey: loginQueryKey,
    queryFn: () => api.get<LoginState>(LOGIN_PATH),
    refetchInterval: (query) => (isInProgress(query.state.data) ? 1000 : false),
  })
}

function useLoginMutation<T>(
  mutationFn: (variables: T) => Promise<LoginState>
) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSuccess: async (state) => {
      // Drop any in-flight poll so it can't overwrite the newer state.
      await queryClient.cancelQueries({ queryKey: loginQueryKey })
      queryClient.setQueryData(loginQueryKey, state)
    },
  })
}

export function useStartTelegramLogin() {
  return useLoginMutation(() => api.post<LoginState>(LOGIN_PATH))
}

export function useSubmitTelegramPassword() {
  return useLoginMutation((password: string) =>
    api.post<LoginState>(`${LOGIN_PATH}/password`, { password })
  )
}

export function useCancelTelegramLogin() {
  return useLoginMutation(async () => {
    await api.delete(LOGIN_PATH)
    return { state: "idle" }
  })
}

export function useDisconnectTelegram() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api.delete("/api/telegram"),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: loginQueryKey })
      return queryClient.invalidateQueries({ queryKey: meQueryKey })
    },
  })
}
