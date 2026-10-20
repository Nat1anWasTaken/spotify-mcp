import { ChevronDownIcon, PencilIcon } from "lucide-react"
import { Link } from "react-router"

import { JsonBlock } from "@/components/json-block"
import { RelativeTime } from "@/components/relative-time"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { useNow } from "@/hooks/use-now"
import { useDecideToolCall } from "@/hooks/use-tool-calls"
import type { ToolCall } from "@/lib/api"
import { formatCountdown } from "@/lib/format"
import { MessagePreview, messageOf } from "@/pages/outbox/message"

export function OutboxCard({ call }: { call: ToolCall }) {
  const now = useNow()
  const approve = useDecideToolCall("approve")
  const discard = useDecideToolCall("discard")
  const deciding = approve.isPending || discard.isPending
  // Only a hint: the browser clock may be off, so the server decides whether
  // the call has expired (a late decision fails with a toast and a refetch).
  const msLeft = call.outbox_expires_at
    ? Date.parse(call.outbox_expires_at) - now
    : null

  return (
    <Card>
      <CardHeader>
        <CardTitle>{call.tool}</CardTitle>
        <CardDescription>
          {call.session_name} · <RelativeTime value={call.created_at} />
        </CardDescription>
        {msLeft !== null && (
          <CardAction>
            <Badge variant="outline">
              {msLeft > 0
                ? `Expires in ${formatCountdown(msLeft)}`
                : "Expiring"}
            </Badge>
          </CardAction>
        )}
      </CardHeader>
      <CardContent>
        <ToolCallArguments call={call} />
      </CardContent>
      <CardFooter className="gap-2">
        <Button
          disabled={deciding}
          onClick={() => approve.mutate({ id: call.id })}
        >
          Approve
        </Button>
        <Button
          variant="outline"
          disabled={deciding}
          onClick={() => discard.mutate({ id: call.id })}
        >
          Discard
        </Button>
        <Button
          variant="ghost"
          className="ml-auto"
          render={<Link to={`/outbox/${call.id}`} />}
          nativeButton={false}
        >
          <PencilIcon />
          Review & edit
        </Button>
      </CardFooter>
    </Card>
  )
}

/** Messages get a formatted preview; everything else is shown as JSON. */
function ToolCallArguments({ call }: { call: ToolCall }) {
  const { peer } = call.arguments
  const message = messageOf(call.arguments)
  if (!message) {
    return <JsonBlock value={call.arguments} />
  }

  return (
    <div className="flex flex-col gap-3">
      {peer !== undefined && (
        <p>
          To <strong>{String(peer)}</strong>
        </p>
      )}
      <MessagePreview
        text={message.text}
        parseMode={message.parseMode}
        className="border-l-2 pl-3"
      />
      <Collapsible>
        <CollapsibleTrigger
          render={<Button variant="ghost" size="sm" className="self-start" />}
        >
          All arguments
          <ChevronDownIcon />
        </CollapsibleTrigger>
        <CollapsibleContent>
          <JsonBlock value={call.arguments} />
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}
