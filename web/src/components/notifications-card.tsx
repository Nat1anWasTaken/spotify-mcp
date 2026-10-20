import { BellIcon, BellOffIcon, ShareIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  useDisablePush,
  useEnablePush,
  usePushSubscription,
  useTestPush,
} from "@/hooks/use-push"
import { pushSupport } from "@/lib/push"

/** Turns outbox notifications on or off for this browser. */
export function NotificationsCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Notifications</CardTitle>
        <CardDescription>
          Get a notification on this device when an agent puts an action in your
          outbox, and tap it to review the request.
        </CardDescription>
      </CardHeader>
      <NotificationsSetting />
    </Card>
  )
}

function NotificationsSetting() {
  const support = pushSupport()
  const subscription = usePushSubscription()
  const enable = useEnablePush()
  const disable = useDisablePush()
  const test = useTestPush()

  if (support === "needs-install") {
    return (
      <CardContent className="text-sm text-muted-foreground">
        On iPhone and iPad, notifications only work in the installed app: tap{" "}
        <ShareIcon className="inline size-4" aria-label="Share" /> then{" "}
        <strong>Add to Home Screen</strong>, and open Telegram MCP from there.
      </CardContent>
    )
  }
  if (support === "unsupported") {
    return (
      <CardContent className="text-sm text-muted-foreground">
        This browser doesn't support push notifications.
      </CardContent>
    )
  }
  if (subscription.isPending) {
    return null
  }
  if (!subscription.data) {
    return (
      <CardFooter>
        <Button disabled={enable.isPending} onClick={() => enable.mutate()}>
          <BellIcon />
          Enable notifications
        </Button>
      </CardFooter>
    )
  }
  const active = subscription.data
  return (
    <CardFooter className="flex-wrap gap-2">
      <Button
        variant="outline"
        disabled={test.isPending}
        onClick={() => test.mutate()}
      >
        Send a test
      </Button>
      <Button
        variant="ghost"
        disabled={disable.isPending}
        onClick={() => disable.mutate(active)}
      >
        <BellOffIcon />
        Turn off on this device
      </Button>
    </CardFooter>
  )
}
