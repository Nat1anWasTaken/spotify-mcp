import { useState } from "react"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useUpdateSession } from "@/hooks/use-sessions"
import type { AgentSession, PeerScope } from "@/lib/api"
import { PeerPicker } from "@/pages/sessions/peer-picker"

type ScopeMode = PeerScope["mode"]

const SCOPE_MODES: { value: ScopeMode; label: string }[] = [
  { value: "all", label: "All chats" },
  { value: "allow", label: "Only selected chats" },
  { value: "deny", label: "All except selected chats" },
]

export function SessionChatAccessCard({ session }: { session: AgentSession }) {
  const update = useUpdateSession(session.id)
  const scope = session.peer_scope
  // A mode change waiting for the user to confirm clearing the selection.
  const [pendingMode, setPendingMode] = useState<ScopeMode | null>(null)

  function setScope(patch: Partial<PeerScope>) {
    update.mutate({ peer_scope: { ...scope, ...patch } })
  }

  // An allow-list and a block-list mean opposite things, so a mode change
  // never carries the selected chats over.
  function changeMode(mode: ScopeMode) {
    if (scope.peers.length > 0) {
      setPendingMode(mode)
    } else {
      setScope({ mode })
    }
  }

  function confirmModeChange() {
    if (pendingMode) {
      setScope({ mode: pendingMode, peers: [] })
    }
    setPendingMode(null)
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Chat access</CardTitle>
        <CardDescription>
          Which chats, groups and channels this agent may read or act on.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field>
            <FieldLabel htmlFor="scope-mode">Access</FieldLabel>
            <Select
              items={SCOPE_MODES}
              value={scope.mode}
              onValueChange={(mode) => mode && changeMode(mode)}
            >
              <SelectTrigger id="scope-mode" className="w-full max-w-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SCOPE_MODES.map((mode) => (
                  <SelectItem key={mode.value} value={mode.value}>
                    {mode.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          {scope.mode !== "all" && (
            <Field>
              <FieldLabel htmlFor="scope-peers">
                {scope.mode === "allow" ? "Allowed chats" : "Blocked chats"}
              </FieldLabel>
              <PeerPicker
                id="scope-peers"
                value={scope.peers}
                onValueChange={(peers) => setScope({ peers })}
              />
            </Field>
          )}
        </FieldGroup>
      </CardContent>
      <AlertDialog
        open={pendingMode !== null}
        onOpenChange={(open) => !open && setPendingMode(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Clear the selected chats?</AlertDialogTitle>
            <AlertDialogDescription>
              Changing the access mode removes the {scope.peers.length} selected{" "}
              {scope.peers.length === 1 ? "chat" : "chats"}, so an allow-list
              never silently turns into a block-list.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={confirmModeChange}>
              Change mode
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  )
}
