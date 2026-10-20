import { useEffect } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { CircleAlertIcon } from "lucide-react"
import { QRCodeSVG } from "qrcode.react"
import { toast } from "sonner"

import { ErrorAlert } from "@/components/error-alert"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { meQueryKey } from "@/hooks/use-me"
import {
  useCancelTelegramLogin,
  useStartTelegramLogin,
  useTelegramLogin,
} from "@/hooks/use-telegram"
import { TelegramPasswordForm } from "@/pages/overview/telegram-password-form"

/** QR-code login: start → scan → optional 2FA password → linked. */
export function TelegramLinkFlow() {
  const queryClient = useQueryClient()
  const login = useTelegramLogin()
  const start = useStartTelegramLogin()
  const loginState = login.data?.state

  useEffect(() => {
    if (loginState === "success") {
      void queryClient.invalidateQueries({ queryKey: meQueryKey })
    }
  }, [loginState, queryClient])

  function startLogin() {
    start.mutate(undefined, { onError: (error) => toast.error(error.message) })
  }

  if (login.isPending) {
    return <Skeleton className="h-9 w-32" />
  }
  if (login.isError) {
    return (
      <ErrorAlert title="Couldn't load the login status" error={login.error} />
    )
  }

  const state = login.data
  switch (state.state) {
    case "idle":
      return (
        <Button onClick={startLogin} disabled={start.isPending}>
          {start.isPending && <Spinner />}
          Link Telegram
        </Button>
      )
    case "waiting_scan":
      return (
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-6">
            <QRCodeSVG value={state.qr_url} size={192} marginSize={2} />
            <ol className="flex list-decimal flex-col gap-1 pl-5 text-sm">
              <li>Open Telegram on your phone.</li>
              <li>Go to Settings → Devices → Link Desktop Device.</li>
              <li>Point your phone at this QR code.</li>
            </ol>
          </div>
          <CancelLoginButton />
        </div>
      )
    case "password_required":
      return (
        <div className="flex flex-col gap-4">
          <TelegramPasswordForm hint={state.password_hint} />
          <CancelLoginButton />
        </div>
      )
    case "success":
      return (
        <div className="flex items-center gap-2 text-sm">
          <Spinner />
          Telegram linked. Loading your account…
        </div>
      )
    case "failed":
      return (
        <div className="flex flex-col gap-4">
          <Alert variant="destructive">
            <CircleAlertIcon />
            <AlertTitle>Linking failed</AlertTitle>
            <AlertDescription>{state.error}</AlertDescription>
          </Alert>
          <Button onClick={startLogin} disabled={start.isPending}>
            {start.isPending && <Spinner />}
            Try again
          </Button>
        </div>
      )
  }
}

function CancelLoginButton() {
  const cancel = useCancelTelegramLogin()

  return (
    <Button
      variant="outline"
      className="self-start"
      disabled={cancel.isPending}
      onClick={() =>
        cancel.mutate(undefined, {
          onError: (error) => toast.error(error.message),
        })
      }
    >
      Cancel
    </Button>
  )
}
