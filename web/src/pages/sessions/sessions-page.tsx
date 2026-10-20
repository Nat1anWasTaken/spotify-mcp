import { BotIcon } from "lucide-react"
import { Link } from "react-router"

import { ErrorAlert } from "@/components/error-alert"
import { PageHeader } from "@/components/page-header"
import { RelativeTime } from "@/components/relative-time"
import { SessionStatusBadges } from "@/components/session-status-badges"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { useSessions } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"

export function SessionsPage() {
  return (
    <>
      <PageHeader
        title="Sessions"
        description="AI agents connected to your Telegram account."
      />
      <SessionsContent />
    </>
  )
}

function SessionsContent() {
  const sessions = useSessions()

  if (sessions.isPending) {
    return <Skeleton className="h-40 w-full" />
  }
  if (sessions.isError) {
    return <ErrorAlert title="Couldn't load sessions" error={sessions.error} />
  }
  if (sessions.data.length === 0) {
    return <NoSessions />
  }
  return <SessionsTable sessions={sessions.data} />
}

function SessionsTable({ sessions }: { sessions: AgentSession[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Name</TableHead>
          <TableHead>Client</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Last used</TableHead>
          <TableHead>Outbox</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {sessions.map((session) => (
          <TableRow key={session.id}>
            <TableCell>
              <Button
                variant="link"
                className="px-0"
                render={<Link to={`/sessions/${session.id}`} />}
                nativeButton={false}
              >
                {session.name}
              </Button>
            </TableCell>
            <TableCell>
              {session.client_name ?? session.origin ?? session.client_id}
            </TableCell>
            <TableCell>
              <SessionStatusBadges session={session} />
            </TableCell>
            <TableCell>
              {session.last_used_at ? (
                <RelativeTime value={session.last_used_at} />
              ) : (
                "Never"
              )}
            </TableCell>
            <TableCell>
              {session.outbox_count > 0 && (
                <Badge render={<Link to="/outbox" />}>
                  {session.outbox_count}
                </Badge>
              )}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}

function NoSessions() {
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <BotIcon />
        </EmptyMedia>
        <EmptyTitle>No agents connected</EmptyTitle>
        <EmptyDescription>
          Add the MCP server URL to an AI client and approve the connection to
          see it here.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button render={<Link to="/connect" />} nativeButton={false}>
          Connect an agent
        </Button>
      </EmptyContent>
    </Empty>
  )
}
