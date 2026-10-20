import { cn } from "cn"

import { FormattedMessage } from "@/components/formatted-message"
import { LinkPreviewCard } from "@/components/link-preview-card"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { useDebouncedValue } from "@/hooks/use-debounced-value"
import { useFormattedText } from "@/hooks/use-formatted-text"
import type { ParseMode } from "@/lib/api"
import { messageLinks } from "@/lib/links"

/** The text a call sends, and how Telegram will format it. */
export type OutgoingMessage = {
  field: "text" | "caption"
  text: string
  parseMode: ParseMode
  /** Telegram attaches a preview of the first link. */
  linkPreview: boolean
}

export const PARSE_MODES: { value: ParseMode; label: string }[] = [
  { value: "markdown", label: "Markdown" },
  { value: "html", label: "HTML" },
  { value: "plain", label: "Plain text" },
]

/** Arguments formatted with the call's `parse_mode` before sending. */
export const MESSAGE_FIELDS = ["text", "caption"] as const

export function parseModeOf(value: unknown): ParseMode {
  return PARSE_MODES.find((mode) => mode.value === value)?.value ?? "markdown"
}

/** The message in a call's arguments: a message's text or a file's caption. */
export function messageOf(
  args: Record<string, unknown>
): OutgoingMessage | null {
  for (const field of MESSAGE_FIELDS) {
    const text = args[field]
    if (typeof text === "string") {
      return {
        field,
        text,
        parseMode: parseModeOf(args.parse_mode),
        // Files show themselves rather than a link preview.
        linkPreview: field === "text" && args.link_preview !== false,
      }
    }
  }
  return null
}

/** The message as Telegram will show it. */
export function MessagePreview({
  text,
  parseMode,
  className,
}: {
  text: string
  parseMode: ParseMode
  className?: string
}) {
  const formatted = useFormattedText(text, parseMode)

  if (formatted.isPending) {
    return <Skeleton className={cn("h-16 w-full", className)} />
  }
  if (formatted.isError) {
    return (
      <p className={cn("text-sm text-destructive", className)}>
        Couldn't preview the formatting.
      </p>
    )
  }
  if (formatted.data.text.trim() === "") {
    return (
      <p className={cn("text-sm text-muted-foreground", className)}>
        Nothing to send yet.
      </p>
    )
  }
  return (
    <FormattedMessage
      value={formatted.data}
      className={cn(
        "transition-opacity",
        formatted.isPlaceholderData && "opacity-60",
        className
      )}
    />
  )
}

function linksDescription(message: OutgoingMessage, count: number) {
  if (count === 0) return "No links in this message."
  if (message.linkPreview) return "Telegram previews the first link."
  if (message.field === "text") return "Link previews are off for this message."
  return "Check where each link leads before sending."
}

/** Every link in the message, previewed. */
export function MessageLinks({
  message,
  className,
}: {
  message: OutgoingMessage
  className?: string
}) {
  const formatted = useFormattedText(message.text, message.parseMode)
  // Settled for longer than the preview, so half-typed links aren't fetched.
  const links = useDebouncedValue(
    formatted.data ? messageLinks(formatted.data).join("\n") : "",
    1000
  )
    .split("\n")
    .filter(Boolean)

  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle>
          Links
          {links.length > 0 && (
            <span className="ml-2 font-normal text-muted-foreground">
              {links.length}
            </span>
          )}
        </CardTitle>
        <CardDescription>
          {linksDescription(message, links.length)}
        </CardDescription>
      </CardHeader>
      {links.length > 0 && (
        <CardContent className="flex flex-col gap-3">
          {links.map((url, index) => (
            <LinkPreviewCard
              key={url}
              url={url}
              telegramPreview={message.linkPreview && index === 0}
            />
          ))}
        </CardContent>
      )}
    </Card>
  )
}
