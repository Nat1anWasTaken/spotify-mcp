import { Badge } from "@/components/ui/badge"
import type { AgentSession } from "@/lib/api"

type SessionStatusBadgesProps = {
  session: Pick<AgentSession, "enabled" | "read_only" | "use_outbox">
}

export function SessionStatusBadges({ session }: SessionStatusBadgesProps) {
  return (
    <div className="flex flex-wrap gap-1">
      {session.enabled ? (
        <Badge variant="secondary">Enabled</Badge>
      ) : (
        <Badge variant="destructive">Disabled</Badge>
      )}
      {session.read_only && <Badge variant="outline">Read-only</Badge>}
      {session.use_outbox && <Badge variant="outline">Outbox</Badge>}
    </div>
  )
}
