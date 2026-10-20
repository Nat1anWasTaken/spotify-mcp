import { cn } from "cn"
import type { ReactNode } from "react"

import type { FormattedText, TextEntity } from "@/lib/api"
import { splitBareUrls } from "@/lib/links"

function ExternalLink({
  href,
  title,
  children,
}: {
  href: string
  title?: string
  children: ReactNode
}) {
  return (
    <a
      href={href}
      title={title}
      target="_blank"
      rel="noreferrer"
      className="text-link underline underline-offset-2"
    >
      {children}
    </a>
  )
}

/** A message rendered with Telegram's formatting entities. */
export function FormattedMessage({
  value,
  className,
}: {
  value: FormattedText
  className?: string
}) {
  const entities = [...value.entities].sort(
    (a, b) => a.offset - b.offset || b.length - a.length
  )
  return (
    <div className={cn("break-words whitespace-pre-wrap", className)}>
      {renderRange(value.text, entities, 0, value.text.length, true)}
    </div>
  )
}

/**
 * Renders `text[start, end)` with `entities`, sorted by offset and then
 * outermost first. An entity that overlaps its parent's end is clipped.
 */
function renderRange(
  text: string,
  entities: TextEntity[],
  start: number,
  end: number,
  linkify: boolean
): ReactNode[] {
  const nodes: ReactNode[] = []
  let position = start
  let index = 0
  const plain = (to: number) => {
    if (to > position) {
      nodes.push(...renderPlain(text.slice(position, to), linkify, position))
    }
  }

  while (index < entities.length) {
    const entity = entities[index]
    const from = Math.max(entity.offset, position)
    const to = Math.min(entity.offset + entity.length, end)
    let next = index + 1
    while (next < entities.length && entities[next].offset < to) {
      next++
    }
    if (to > from) {
      plain(from)
      const inner = renderRange(
        text,
        entities.slice(index + 1, next),
        from,
        to,
        linkify && !["code", "pre", "url", "text_url"].includes(entity.type)
      )
      nodes.push(wrap(entity, inner, text.slice(from, to), from))
      position = to
    }
    index = next
  }
  plain(end)
  return nodes
}

function renderPlain(text: string, linkify: boolean, key: number) {
  if (!linkify) {
    return [text]
  }
  return splitBareUrls(text).map((part, index) =>
    part.url ? (
      <ExternalLink key={`${key}-${index}`} href={part.text}>
        {part.text}
      </ExternalLink>
    ) : (
      part.text
    )
  )
}

function wrap(
  entity: TextEntity,
  children: ReactNode[],
  text: string,
  key: number
): ReactNode {
  switch (entity.type) {
    case "bold":
      return <strong key={key}>{children}</strong>
    case "italic":
      return <em key={key}>{children}</em>
    case "underline":
      return <u key={key}>{children}</u>
    case "strike":
      return <s key={key}>{children}</s>
    case "spoiler":
      return (
        <span
          key={key}
          title="Spoiler"
          className="rounded-sm bg-muted-foreground/30 blur-[3px] transition hover:blur-none"
        >
          {children}
        </span>
      )
    case "code":
      return (
        <code
          key={key}
          className="rounded bg-muted px-1 font-mono text-[0.9em]"
        >
          {children}
        </code>
      )
    case "pre":
      return (
        <pre
          key={key}
          className="my-1 overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs"
        >
          {children}
        </pre>
      )
    case "blockquote":
      return (
        <blockquote key={key} className="my-1 border-l-2 border-link pl-2">
          {children}
        </blockquote>
      )
    case "url":
      return (
        <ExternalLink key={key} href={text}>
          {children}
        </ExternalLink>
      )
    case "text_url":
      // Agent-written: only link schemes that are safe to open.
      return /^(https?|tg|mailto):/i.test(entity.url) ? (
        <ExternalLink key={key} href={entity.url} title={entity.url}>
          {children}
        </ExternalLink>
      ) : (
        <span key={key} title={entity.url} className="text-link">
          {children}
        </span>
      )
    case "mention":
      return (
        <span key={key} className="text-link">
          {children}
        </span>
      )
  }
}
