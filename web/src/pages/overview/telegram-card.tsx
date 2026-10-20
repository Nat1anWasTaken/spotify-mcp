import { toast } from "sonner"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { useSignedInMe } from "@/hooks/use-me"
import { useDisconnectTelegram } from "@/hooks/use-telegram"
import type { TelegramStatus } from "@/lib/api"
import { TelegramLinkFlow } from "@/pages/overview/telegram-link-flow"

export function TelegramCard() {
  const { telegram } = useSignedInMe()

  return (
    <Card>
      <CardHeader>
        <CardTitle>Telegram</CardTitle>
        <CardDescription>
          {telegram.linked
            ? "Agents act as this Telegram account."
            : "Link your Telegram account so agents can use it."}
        </CardDescription>
        {telegram.linked && (
          <CardAction>
            <DisconnectTelegramButton />
          </CardAction>
        )}
      </CardHeader>
      <CardContent>
        {telegram.account ? (
          <TelegramAccountDetails account={telegram.account} />
        ) : (
          <TelegramLinkFlow />
        )}
      </CardContent>
    </Card>
  )
}

type TelegramAccount = NonNullable<TelegramStatus["account"]>

function TelegramAccountDetails({ account }: { account: TelegramAccount }) {
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
      <dt className="text-muted-foreground">Name</dt>
      <dd>{account.display_name}</dd>
      {account.username && (
        <>
          <dt className="text-muted-foreground">Username</dt>
          <dd>@{account.username}</dd>
        </>
      )}
      {account.phone && (
        <>
          <dt className="text-muted-foreground">Phone</dt>
          <dd>{account.phone}</dd>
        </>
      )}
      <dt className="text-muted-foreground">User ID</dt>
      <dd>{account.tg_user_id}</dd>
    </dl>
  )
}

function DisconnectTelegramButton() {
  const disconnect = useDisconnectTelegram()

  return (
    <AlertDialog>
      <AlertDialogTrigger render={<Button variant="destructive" size="sm" />}>
        Disconnect
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Disconnect Telegram?</AlertDialogTitle>
          <AlertDialogDescription>
            This signs the dashboard out of your Telegram account and deletes
            the stored session. Connected agents won't be able to do anything
            until you link Telegram again.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={disconnect.isPending}
            onClick={() =>
              disconnect.mutate(undefined, {
                onSuccess: () => toast.success("Telegram disconnected"),
                onError: (error) => toast.error(error.message),
              })
            }
          >
            Disconnect
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
