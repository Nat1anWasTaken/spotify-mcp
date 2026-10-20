import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

import { api } from "@/lib/api"
import {
  currentSubscription,
  pushSupport,
  subscribe,
  unsubscribe,
} from "@/lib/push"

const subscriptionQueryKey = ["push-subscription"] as const

/** This browser's push subscription, or null when it has none. */
export function usePushSubscription() {
  return useQuery({
    queryKey: subscriptionQueryKey,
    queryFn: currentSubscription,
    enabled: pushSupport() === "supported",
    staleTime: Infinity,
  })
}

export function useEnablePush() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: subscribe,
    onSuccess: (subscription) => {
      queryClient.setQueryData(subscriptionQueryKey, subscription)
      toast.success("Notifications enabled")
    },
    onError: (error) => toast.error(error.message),
  })
}

export function useDisablePush() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: unsubscribe,
    onSuccess: () => {
      queryClient.setQueryData(subscriptionQueryKey, null)
      toast.success("Notifications turned off on this device")
    },
    onError: (error) => toast.error(error.message),
  })
}

export function useTestPush() {
  return useMutation({
    mutationFn: () => api.post<{ delivered: number }>("/api/push/test"),
    onSuccess: ({ delivered }) =>
      delivered > 0
        ? toast.success(
            `Sent to ${delivered} device${delivered === 1 ? "" : "s"}`
          )
        : toast.error("No device accepted the notification."),
    onError: (error) => toast.error(error.message),
  })
}
