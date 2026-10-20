import type { FormattedText } from "@/lib/api"

/** Bare URLs, which Telegram links by itself; trailing punctuation excluded. */
const BARE_URL = /\bhttps?:\/\/[^\s<>"]*[^\s<>"'.,;:!?)\]}]/gi

function isWebUrl(url: string) {
  return /^https?:\/\//i.test(url)
}

/** Every web link in a formatted message, in order, without duplicates. */
export function messageLinks({ text, entities }: FormattedText): string[] {
  const found: { offset: number; url: string }[] = []
  for (const entity of entities) {
    if (entity.type === "text_url" && isWebUrl(entity.url)) {
      found.push({ offset: entity.offset, url: entity.url })
    }
  }
  for (const match of text.matchAll(BARE_URL)) {
    found.push({ offset: match.index, url: match[0] })
  }
  found.sort((a, b) => a.offset - b.offset)
  return [...new Set(found.map((link) => link.url))]
}

/** Splits text around bare URLs, for linking them in a preview. */
export function splitBareUrls(text: string): { text: string; url: boolean }[] {
  const parts: { text: string; url: boolean }[] = []
  let position = 0
  for (const match of text.matchAll(BARE_URL)) {
    if (match.index > position) {
      parts.push({ text: text.slice(position, match.index), url: false })
    }
    parts.push({ text: match[0], url: true })
    position = match.index + match[0].length
  }
  if (position < text.length) {
    parts.push({ text: text.slice(position), url: false })
  }
  return parts
}
