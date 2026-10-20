import { SendIcon } from "lucide-react"

import { ErrorAlert } from "@/components/error-alert"
import { NotificationsCard } from "@/components/notifications-card"
import { PageHeader } from "@/components/page-header"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { useOutbox } from "@/hooks/use-tool-calls"
import { OutboxCard } from "@/pages/outbox/outbox-card"

export function OutboxPage() {
  return (
    <>
      <PageHeader
        title="Outbox"
        description="Write actions your agents queued, waiting for you to approve or discard them."
      />
      <QueuedCalls />
      <NotificationsCard />
    </>
  )
}

function QueuedCalls() {
  const queued = useOutbox()

  if (queued.isPending) {
    return <Skeleton className="h-48 w-full" />
  }
  if (queued.isError) {
    return <ErrorAlert title="Couldn't load the outbox" error={queued.error} />
  }
  if (queued.data.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <SendIcon />
          </EmptyMedia>
          <EmptyTitle>Your outbox is empty</EmptyTitle>
          <EmptyDescription>
            Sessions with “Use the outbox” on queue their writes here instead of
            sending, editing or deleting anything themselves.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }
  return (
    <div className="flex flex-col gap-4">
      {queued.data.map((call) => (
        <OutboxCard key={call.id} call={call} />
      ))}
    </div>
  )
}
