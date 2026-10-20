import { cn } from "cn"
import { useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import { useLinkPreview } from "@/hooks/use-link-preview"

function hostOf(url: string) {
  try {
    return new URL(url).host
  } catch {
    return url
  }
}

/** A link with its page's title, description and image, as a card. */
export function LinkPreviewCard({
  url,
  telegramPreview,
}: {
  url: string
  /** Telegram will attach this link's preview to the message. */
  telegramPreview?: boolean
}) {
  const preview = useLinkPreview(url)
  const [imageFailed, setImageFailed] = useState(false)
  const data = preview.data
  const finalUrl = data?.final_url ?? url
  const redirected = data?.final_url && hostOf(data.final_url) !== hostOf(url)

  return (
    <a
      href={url}
      target="_blank"
      rel="noreferrer"
      className={cn(
        "flex flex-col overflow-hidden rounded-lg border text-sm transition-colors hover:bg-muted/50",
        telegramPreview && "border-link"
      )}
    >
      {preview.isPending && <Skeleton className="aspect-[1.91] rounded-none" />}
      {data?.image && !imageFailed && (
        <img
          src={data.image}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="aspect-[1.91] w-full bg-muted object-cover"
          onError={() => setImageFailed(true)}
        />
      )}
      <div className="flex flex-col gap-1 p-3">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className="truncate">
            {data?.site_name ?? hostOf(finalUrl)}
          </span>
          {telegramPreview && (
            <Badge variant="outline" className="ml-auto shrink-0">
              Telegram's preview
            </Badge>
          )}
        </div>
        {preview.isPending ? (
          <>
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-3 w-full" />
          </>
        ) : data?.title || data?.description ? (
          <>
            {data.title && (
              <p className="line-clamp-2 font-medium">{data.title}</p>
            )}
            {data.description && (
              <p className="line-clamp-3 text-muted-foreground">
                {data.description}
              </p>
            )}
          </>
        ) : (
          <p className="text-muted-foreground">
            {data?.final_url ? "No preview" : "Couldn't load this page"}
          </p>
        )}
        <p className="truncate font-mono text-xs text-muted-foreground">
          {redirected ? `${url} → ${hostOf(finalUrl)}` : url}
        </p>
      </div>
    </a>
  )
}
