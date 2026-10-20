import { Link } from "react-router"

import { CopyField } from "@/components/copy-field"
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
import { MCP_URL } from "@/lib/mcp-url"
import { LinkCard } from "@/pages/overview/link-card"
import { NowPlayingCard } from "@/pages/overview/now-playing-card"
import { ProfileCard } from "@/pages/overview/profile-card"

export function OverviewPage() {
  return (
    <>
      <PageHeader title="Overview" />
      <div className="grid gap-4 md:grid-cols-2">
        <ProfileCard />
        <NowPlayingCard />
      </div>
      <LinkCard />
      <SessionsCard />
      <McpUrlCard />
    </>
  )
}

function SessionsCard() {
  const count = useSessions().data?.length

  return (
    <Card>
      <CardHeader>
        <CardDescription>Connected agents</CardDescription>
        <CardTitle>
          {count === undefined ? <Skeleton className="h-6 w-8" /> : count}
        </CardTitle>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            render={<Link to="/sessions" />}
            nativeButton={false}
          >
            View sessions
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
          Add this URL as a custom connector in claude.ai, ChatGPT, Claude Code
          or another MCP client.
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
