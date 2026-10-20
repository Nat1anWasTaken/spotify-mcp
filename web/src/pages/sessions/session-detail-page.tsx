import { ArrowLeftIcon } from "lucide-react"
import { Link } from "react-router"

import { ErrorAlert } from "@/components/error-alert"
import { PageHeader } from "@/components/page-header"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { useRequiredParam } from "@/hooks/use-required-param"
import { useSession } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"
import { formatDateTime } from "@/lib/format"
import { DisconnectSessionButton } from "@/pages/sessions/disconnect-session-button"
import { SessionNameForm } from "@/pages/sessions/session-name-form"
import { SessionPermissionsCard } from "@/pages/sessions/session-permissions-card"
import { SessionRecentActivityCard } from "@/pages/sessions/session-recent-activity-card"
import { SessionToolsCard } from "@/pages/sessions/session-tools-card"

export function SessionDetailPage() {
  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="self-start"
        render={<Link to="/sessions" />}
        nativeButton={false}
      >
        <ArrowLeftIcon />
        Sessions
      </Button>
      <SessionDetailContent />
    </>
  )
}

function SessionDetailContent() {
  const session = useSession(useRequiredParam("id"))

  if (session.isPending) {
    return <Skeleton className="h-96 w-full" />
  }
  if (session.isError) {
    return (
      <ErrorAlert title="Couldn't load this session" error={session.error} />
    )
  }
  return <SessionDetail session={session.data} />
}

function SessionDetail({ session }: { session: AgentSession }) {
  const details = [
    session.client_name ?? session.client_id,
    session.origin,
    `connected ${formatDateTime(session.created_at)}`,
  ]

  return (
    <>
      <PageHeader
        title={session.name}
        description={details.filter(Boolean).join(" · ")}
        actions={<DisconnectSessionButton session={session} />}
      />
      <SessionNameForm session={session} />
      <SessionPermissionsCard session={session} />
      <SessionToolsCard session={session} />
      <SessionRecentActivityCard sessionId={session.id} />
    </>
  )
}
