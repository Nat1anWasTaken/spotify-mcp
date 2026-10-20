import { Link } from "react-router"

import { ErrorAlert } from "@/components/error-alert"
import { RelativeTime } from "@/components/relative-time"
import { ToolCallStatusBadge } from "@/components/tool-call-status-badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table"
import { useRecentToolCalls } from "@/hooks/use-tool-calls"
import { withQuery } from "@/lib/api"

export function SessionRecentActivityCard({
  sessionId,
}: {
  sessionId: string
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent activity</CardTitle>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            render={
              <Link to={withQuery("/activity", { session_id: sessionId })} />
            }
            nativeButton={false}
          >
            View all
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        <RecentToolCalls sessionId={sessionId} />
      </CardContent>
    </Card>
  )
}

function RecentToolCalls({ sessionId }: { sessionId: string }) {
  const calls = useRecentToolCalls(sessionId)

  if (calls.isPending) {
    return <Skeleton className="h-24 w-full" />
  }
  if (calls.isError) {
    return <ErrorAlert title="Couldn't load activity" error={calls.error} />
  }
  if (calls.data.length === 0) {
    return <p className="text-sm text-muted-foreground">No tool calls yet.</p>
  }
  return (
    <Table>
      <TableBody>
        {calls.data.map((call) => (
          <TableRow key={call.id}>
            <TableCell>{call.tool}</TableCell>
            <TableCell>
              <ToolCallStatusBadge status={call.status} />
            </TableCell>
            <TableCell>
              <RelativeTime value={call.created_at} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
