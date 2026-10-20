import type { ComponentProps } from "react"

import { Badge } from "@/components/ui/badge"
import type { ToolCallStatus } from "@/lib/api"

type BadgeVariant = ComponentProps<typeof Badge>["variant"]

export const TOOL_CALL_STATUSES: Record<
  ToolCallStatus,
  { label: string; variant: BadgeVariant }
> = {
  running: { label: "Running", variant: "outline" },
  ok: { label: "OK", variant: "secondary" },
  error: { label: "Error", variant: "destructive" },
  denied: { label: "Denied", variant: "destructive" },
  queued: { label: "In outbox", variant: "default" },
  discarded: { label: "Discarded", variant: "outline" },
  expired: { label: "Expired", variant: "outline" },
}

export function ToolCallStatusBadge({ status }: { status: ToolCallStatus }) {
  const { label, variant } = TOOL_CALL_STATUSES[status]
  return <Badge variant={variant}>{label}</Badge>
}
