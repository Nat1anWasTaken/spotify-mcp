const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
})

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "long" })

const relativeTimeFormat = new Intl.RelativeTimeFormat(undefined, {
  numeric: "auto",
})

const RELATIVE_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 60 * 60],
  ["month", 30 * 24 * 60 * 60],
  ["day", 24 * 60 * 60],
  ["hour", 60 * 60],
  ["minute", 60],
  ["second", 1],
]

export function formatDateTime(iso: string): string {
  return dateTimeFormat.format(new Date(iso))
}

/** "April 20, 2027" */
export function formatDate(iso: string): string {
  return dateFormat.format(new Date(iso))
}

/** "3 minutes ago", "yesterday", … */
export function formatRelativeTime(iso: string): string {
  const seconds = (new Date(iso).getTime() - Date.now()) / 1000
  const [unit, unitSeconds] = RELATIVE_UNITS.find(
    ([, unitSeconds]) => Math.abs(seconds) >= unitSeconds
  ) ?? ["second", 1]
  return relativeTimeFormat.format(Math.round(seconds / unitSeconds), unit)
}

export function formatDuration(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`
}

export function formatJson(value: unknown): string {
  return JSON.stringify(value, null, 2)
}

/** Up to two initials for an avatar fallback: "Ada Lovelace" → "AL". */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean)
  const letters =
    words.length > 1
      ? words[0][0] + words[words.length - 1][0]
      : name.slice(0, 2)
  return letters.toUpperCase()
}
