import { useNavigate } from "react-router"
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
import { useDeleteSession } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"

export function DisconnectSessionButton({
  session,
}: {
  session: AgentSession
}) {
  const navigate = useNavigate()
  const deleteSession = useDeleteSession(session.id)

  function disconnect() {
    deleteSession.mutate(undefined, {
      onSuccess: () => {
        toast.success(`Disconnected ${session.name}`)
        void navigate("/sessions")
      },
      onError: (error) => toast.error(error.message),
    })
  }

  return (
    <AlertDialog>
      <AlertDialogTrigger render={<Button variant="destructive" />}>
        Disconnect
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Disconnect {session.name}?</AlertDialogTitle>
          <AlertDialogDescription>
            The agent's access is revoked immediately. Its activity log is kept.
            To use it again, reconnect it from the AI client.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={deleteSession.isPending}
            onClick={disconnect}
          >
            Disconnect
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
