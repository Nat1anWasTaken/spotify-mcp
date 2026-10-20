import { ArrowLeftIcon, RotateCcwIcon } from "lucide-react"
import { useState, type ReactNode } from "react"
import { Link } from "react-router"

import { ErrorAlert } from "@/components/error-alert"
import { JsonBlock } from "@/components/json-block"
import { PageHeader } from "@/components/page-header"
import { RelativeTime } from "@/components/relative-time"
import { ToolCallStatusBadge } from "@/components/tool-call-status-badge"
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
import { Skeleton } from "@/components/ui/skeleton"
import { useNow } from "@/hooks/use-now"
import { useRequiredParam } from "@/hooks/use-required-param"
import { useDecideToolCall, useToolCall } from "@/hooks/use-tool-calls"
import { useTools } from "@/hooks/use-tools"
import type { ToolCallDetail } from "@/lib/api"
import { formatCountdown } from "@/lib/format"
import {
  ArgumentsEditor,
  changedArguments,
  type Arguments,
} from "@/pages/outbox/arguments-editor"
import {
  MessageLinks,
  MessagePreview,
  messageOf,
  type OutgoingMessage,
} from "@/pages/outbox/message"

export function OutboxItemPage() {
  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="self-start"
        render={<Link to="/outbox" />}
        nativeButton={false}
      >
        <ArrowLeftIcon />
        Outbox
      </Button>
      <OutboxItemContent />
    </>
  )
}

function OutboxItemContent() {
  const call = useToolCall(useRequiredParam("id"))
  const tools = useTools()

  if (call.isPending) {
    return <Skeleton className="h-96 w-full" />
  }
  if (call.isError) {
    return <ErrorAlert title="Couldn't load this request" error={call.error} />
  }
  const title =
    tools.data?.find((tool) => tool.name === call.data.tool)?.title ??
    call.data.tool

  return (
    <>
      <PageHeader
        title={title}
        description={
          <>
            {call.data.session_name} · <code>{call.data.tool}</code> ·{" "}
            <RelativeTime value={call.data.created_at} />
          </>
        }
        actions={<ToolCallStatusBadge status={call.data.status} />}
      />
      {call.data.editable_arguments ? (
        // Keyed so a different call starts from its own arguments.
        <ReviewCard
          key={call.data.id}
          call={call.data}
          original={call.data.editable_arguments}
        />
      ) : (
        <OutcomeCard call={call.data} />
      )}
    </>
  )
}

function ReviewCard({
  call,
  original,
}: {
  call: ToolCallDetail
  original: Arguments
}) {
  const now = useNow()
  const approve = useDecideToolCall("approve")
  const discard = useDecideToolCall("discard")
  const [draft, setDraft] = useState(original)
  const [invalid, setInvalid] = useState<ReadonlySet<string>>(new Set())
  const deciding = approve.isPending || discard.isPending
  const edits = changedArguments(original, draft)
  const edited = Object.keys(edits).length > 0
  // Only a hint: the server decides whether the call has expired.
  const msLeft = call.outbox_expires_at
    ? Date.parse(call.outbox_expires_at) - now
    : null

  const message = messageOf(draft)

  const setFieldInvalid = (key: string, isInvalid: boolean) =>
    setInvalid((current) => {
      const next = new Set(current)
      if (isInvalid) {
        next.add(key)
      } else {
        next.delete(key)
      }
      return next
    })

  return (
    <WithLinks message={message}>
      <Card>
        <CardHeader>
          <CardTitle>Review the request</CardTitle>
          <CardDescription>
            Change anything before approving; the agent is told what actually
            ran.
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
          <ArgumentsEditor
            tool={call.tool}
            original={original}
            value={draft}
            onChange={setDraft}
            onInvalidChange={setFieldInvalid}
          />
        </CardContent>
        <CardFooter className="flex-wrap gap-2">
          <Button
            disabled={deciding || invalid.size > 0}
            onClick={() =>
              approve.mutate({
                id: call.id,
                arguments: edited ? edits : undefined,
              })
            }
          >
            {edited ? "Approve with edits" : "Approve"}
          </Button>
          <Button
            variant="outline"
            disabled={deciding}
            onClick={() => discard.mutate({ id: call.id })}
          >
            Discard
          </Button>
          {edited && (
            <Button
              variant="ghost"
              className="ml-auto"
              disabled={deciding}
              onClick={() => {
                setDraft(original)
                setInvalid(new Set())
              }}
            >
              <RotateCcwIcon />
              Undo edits
            </Button>
          )}
        </CardFooter>
      </Card>
    </WithLinks>
  )
}

/** The page's card, with the message's links beside it on wide screens. */
function WithLinks({
  message,
  children,
}: {
  message: OutgoingMessage | null
  children: ReactNode
}) {
  if (!message) {
    return children
  }
  return (
    <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_20rem]">
      {children}
      <MessageLinks
        message={message}
        className="xl:sticky xl:top-4 xl:max-h-[calc(100svh-2rem)] xl:overflow-y-auto"
      />
    </div>
  )
}

/** A call that has left the outbox: what ran, and how it ended. */
function OutcomeCard({ call }: { call: ToolCallDetail }) {
  const message = messageOf(call.arguments)

  return (
    <WithLinks message={message}>
      <Card>
        <CardHeader>
          <CardTitle>{outcomeTitle(call)}</CardTitle>
          {call.error && <CardDescription>{call.error}</CardDescription>}
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {message && (
            <section className="flex flex-col gap-2">
              <h2 className="text-sm font-medium">Message</h2>
              <MessagePreview
                text={message.text}
                parseMode={message.parseMode}
                className="rounded-lg border bg-muted/30 px-3 py-2 text-sm"
              />
            </section>
          )}
          <section className="flex flex-col gap-2">
            <h2 className="text-sm font-medium">
              {call.original_arguments
                ? "Arguments, as you edited them"
                : "Arguments"}
            </h2>
            <JsonBlock value={call.arguments} />
          </section>
          {call.original_arguments && (
            <section className="flex flex-col gap-2">
              <h2 className="text-sm font-medium">The agent's arguments</h2>
              <JsonBlock value={call.original_arguments} />
            </section>
          )}
          {call.result !== null && (
            <section className="flex flex-col gap-2">
              <h2 className="text-sm font-medium">Result</h2>
              <JsonBlock value={call.result} />
            </section>
          )}
        </CardContent>
      </Card>
    </WithLinks>
  )
}

function outcomeTitle(call: ToolCallDetail): string {
  switch (call.status) {
    case "queued":
      return "This request can no longer be edited"
    case "running":
      return "Approved, running…"
    case "ok":
      return "Approved and done"
    case "error":
      return "Approved, but it failed"
    case "denied":
      return "Approved, but the connection's permissions refused it"
    case "discarded":
      return "Discarded"
    case "expired":
      return "Expired before you decided"
  }
}
