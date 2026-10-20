import { Link } from "react-router"

import { CopyField } from "@/components/copy-field"
import { NotificationsCard } from "@/components/notifications-card"
import { PageHeader } from "@/components/page-header"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { useSessions } from "@/hooks/use-sessions"
import { useOutbox } from "@/hooks/use-tool-calls"
import { MCP_URL } from "@/lib/mcp-url"
import { TelegramCard } from "@/pages/overview/telegram-card"

export function OverviewPage() {
  const sessions = useSessions()
  const outbox = useOutbox()

  return (
    <>
      <PageHeader title="Overview" />
      <TelegramCard />
      <div className="grid gap-4 md:grid-cols-2">
        <CountCard
          title="Active sessions"
          count={sessions.data?.length}
          to="/sessions"
          linkLabel="View sessions"
        />
        <CountCard
          title="In your outbox"
          count={outbox.data?.length}
          to="/outbox"
          linkLabel="Review"
        />
      </div>
      <NotificationsCard />
      <McpUrlCard />
    </>
  )
}

type CountCardProps = {
  title: string
  count: number | undefined
  to: string
  linkLabel: string
}

function CountCard({ title, count, to, linkLabel }: CountCardProps) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>{title}</CardDescription>
        <CardTitle>
          {count === undefined ? <Skeleton className="h-6 w-8" /> : count}
        </CardTitle>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            render={<Link to={to} />}
            nativeButton={false}
          >
            {linkLabel}
          </Button>
        </CardAction>
      </CardHeader>
    </Card>
  )
}

function McpUrlCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>MCP server URL</CardTitle>
        <CardDescription>
          Add this URL as a custom connector in your AI client.
        </CardDescription>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            render={<Link to="/connect" />}
            nativeButton={false}
          >
            Setup guide
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        <CopyField label="MCP server URL" value={MCP_URL} />
      </CardContent>
    </Card>
  )
}
