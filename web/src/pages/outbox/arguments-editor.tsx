import { cn } from "cn"
import { useState, type ReactNode } from "react"

import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { SourceEditor } from "@/components/source-editor"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { useDebouncedValue } from "@/hooks/use-debounced-value"
import { useFormattedText } from "@/hooks/use-formatted-text"
import { usePeers } from "@/hooks/use-peers"
import type { ParseMode, PeerSummary } from "@/lib/api"
import { formatJson } from "@/lib/format"
import {
  MESSAGE_FIELDS,
  MessagePreview,
  PARSE_MODES,
  parseModeOf,
} from "@/pages/outbox/message"

export type Arguments = Record<string, unknown>

type FieldKind =
  | "peer"
  | "message"
  | "long-text"
  | "text"
  | "number"
  | "boolean"
  | "parse-mode"
  | "json"
  | "read-only"

const PEER_FIELDS = new Set(["peer", "from_peer", "to_peer", "user"])
/** Telegram's limits, after formatting. */
const LONG_TEXT_LIMITS: Record<string, number | undefined> = {
  text: 4096,
  caption: 1024,
  bio: 70,
  about: 255,
}
const NUMBER_FIELDS = new Set(["reply_to", "message_id"])

/** How a field is edited, from its name and the value the agent gave. */
function kindOf(tool: string, key: string, value: unknown): FieldKind {
  if (tool === "send_file" && (key === "url" || key === "base64")) {
    return "read-only"
  }
  if (PEER_FIELDS.has(key)) return "peer"
  if (key === "parse_mode") return "parse-mode"
  if (MESSAGE_FIELDS.some((field) => field === key)) return "message"
  if (key in LONG_TEXT_LIMITS) return "long-text"
  if (NUMBER_FIELDS.has(key) || typeof value === "number") return "number"
  if (typeof value === "boolean") return "boolean"
  if (typeof value === "string") return "text"
  return "json"
}

/** The top-level arguments that differ from the agent's. */
export function changedArguments(original: Arguments, draft: Arguments) {
  return Object.fromEntries(
    Object.entries(draft).filter(
      ([key, value]) => JSON.stringify(value) !== JSON.stringify(original[key])
    )
  )
}

type ArgumentsEditorProps = {
  tool: string
  original: Arguments
  value: Arguments
  onChange: (value: Arguments) => void
  /** Fields whose input doesn't parse yet; approving waits for them. */
  onInvalidChange: (key: string, invalid: boolean) => void
}

/** A form over a call's arguments, one field per argument. */
export function ArgumentsEditor({
  tool,
  original,
  value,
  onChange,
  onInvalidChange,
}: ArgumentsEditorProps) {
  return (
    <FieldGroup>
      {Object.keys(original).map((key) => (
        <ArgumentField
          key={key}
          id={`argument-${key}`}
          name={key}
          kind={kindOf(tool, key, original[key])}
          value={value[key]}
          changed={JSON.stringify(value[key]) !== JSON.stringify(original[key])}
          parseMode={parseModeOf(value.parse_mode)}
          onChange={(next) => onChange({ ...value, [key]: next })}
          onInvalidChange={(invalid) => onInvalidChange(key, invalid)}
        />
      ))}
    </FieldGroup>
  )
}

type ArgumentFieldProps = {
  id: string
  name: string
  kind: FieldKind
  value: unknown
  changed: boolean
  /** How message fields are formatted. */
  parseMode: ParseMode
  onChange: (value: unknown) => void
  onInvalidChange: (invalid: boolean) => void
}

function ArgumentField(props: ArgumentFieldProps) {
  const { id, name, kind, value, changed, onChange } = props
  const label = (
    <FieldLabel htmlFor={id}>
      {name}
      {changed && <span className="text-xs text-primary">edited</span>}
    </FieldLabel>
  )

  switch (kind) {
    case "boolean":
      return (
        <Field orientation="horizontal">
          <FieldContent>{label}</FieldContent>
          <Switch
            id={id}
            checked={value === true}
            onCheckedChange={(checked) => onChange(checked)}
          />
        </Field>
      )
    case "parse-mode":
      return (
        <Field>
          {label}
          <Select
            items={PARSE_MODES}
            value={typeof value === "string" ? value : "markdown"}
            onValueChange={(mode) => mode && onChange(mode)}
          >
            <SelectTrigger id={id} className="w-full max-w-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PARSE_MODES.map((mode) => (
                <SelectItem key={mode.value} value={mode.value}>
                  {mode.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      )
    case "message":
      return <MessageField {...props} label={label} />
    case "long-text": {
      const text = typeof value === "string" ? value : ""
      const limit = LONG_TEXT_LIMITS[name]
      return (
        <Field>
          {label}
          <Textarea
            id={id}
            value={text}
            className="min-h-32"
            onChange={(event) =>
              onChange(
                event.target.value === "" && value === null
                  ? null
                  : event.target.value
              )
            }
          />
          {limit && (
            <FieldDescription
              className={text.length > limit ? "text-destructive" : undefined}
            >
              {text.length} / {limit} characters
            </FieldDescription>
          )}
        </Field>
      )
    }
    case "text":
      return (
        <Field>
          {label}
          <Input
            id={id}
            value={String(value ?? "")}
            onChange={(event) => onChange(event.target.value)}
          />
        </Field>
      )
    case "number":
      return (
        <Field>
          {label}
          <Input
            id={id}
            type="number"
            inputMode="numeric"
            className="max-w-xs"
            value={typeof value === "number" ? value : ""}
            onChange={(event) =>
              onChange(
                event.target.value === "" ? null : Number(event.target.value)
              )
            }
          />
        </Field>
      )
    case "peer":
      return (
        <Field>
          {label}
          <PeerField id={id} value={value} onChange={onChange} />
        </Field>
      )
    case "read-only":
      return (
        <Field>
          {label}
          <Input id={id} value={String(value ?? "")} disabled />
          <FieldDescription>The file itself can't be edited.</FieldDescription>
        </Field>
      )
    case "json":
      return (
        <JsonField
          {...props}
          label={label}
          onInvalidChange={props.onInvalidChange}
        />
      )
  }
}

/** Message markup typed by hand, beside a preview of how it will look. */
function MessageField({
  id,
  name,
  value,
  parseMode,
  label,
  onChange,
}: ArgumentFieldProps & { label: ReactNode }) {
  const [pane, setPane] = useState<"source" | "preview">("source")
  const text = typeof value === "string" ? value : ""
  const formatted = useFormattedText(text, parseMode)
  const limit = LONG_TEXT_LIMITS[name]
  const length = formatted.data?.text.length

  return (
    <Field>
      <div className="flex items-center justify-between gap-2">
        {label}
        <Tabs
          value={pane}
          onValueChange={(next) => setPane(next)}
          className="md:hidden"
        >
          <TabsList>
            <TabsTrigger value="source">Write</TabsTrigger>
            <TabsTrigger value="preview">Preview</TabsTrigger>
          </TabsList>
        </Tabs>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        <SourceEditor
          id={id}
          value={text}
          language={parseMode}
          placeholder={value === null ? "No caption" : undefined}
          className={cn(pane === "preview" && "max-md:hidden")}
          onChange={(next) =>
            // A caption left empty stays absent rather than "".
            onChange(next === "" && value === null ? null : next)
          }
        />
        <MessagePreview
          text={text}
          parseMode={parseMode}
          className={cn(
            "min-h-32 rounded-lg border bg-muted/30 px-3 py-2 text-sm",
            pane === "source" && "max-md:hidden"
          )}
        />
      </div>
      <FieldDescription
        className={
          limit && length !== undefined && length > limit
            ? "text-destructive"
            : undefined
        }
      >
        {length ?? "…"}
        {limit && ` / ${limit}`} characters once formatted
      </FieldDescription>
    </Field>
  )
}

/** A chat: typed as an ID, @username, "me" or t.me link, or searched for. */
function PeerField({
  id,
  value,
  onChange,
}: {
  id: string
  value: unknown
  onChange: (value: unknown) => void
}) {
  const [query, setQuery] = useState("")
  const peers = usePeers(useDebouncedValue(query, 250))

  return (
    <div className="flex flex-col gap-2">
      <Input
        id={id}
        value={String(value ?? "")}
        placeholder="@username, chat ID or me"
        onChange={(event) => onChange(event.target.value)}
      />
      <Combobox
        items={peers.data ?? []}
        filter={null}
        value={null}
        inputValue={query}
        onInputValueChange={setQuery}
        onValueChange={(peer: PeerSummary | null) => {
          if (peer) {
            onChange(peer.username ? `@${peer.username}` : String(peer.id))
            setQuery("")
          }
        }}
        itemToStringLabel={(peer: PeerSummary) => peer.title}
      >
        <ComboboxInput
          className="w-full"
          placeholder="Or search your chats…"
          showTrigger={false}
          aria-label="Search your chats"
        />
        <ComboboxContent>
          <ComboboxEmpty>
            {peers.isError ? "Couldn't load chats." : "No chats found."}
          </ComboboxEmpty>
          <ComboboxList>
            {(peer: PeerSummary) => (
              <ComboboxItem key={peer.id} value={peer}>
                {peer.title}
                {peer.username && (
                  <span className="text-muted-foreground">
                    @{peer.username}
                  </span>
                )}
              </ComboboxItem>
            )}
          </ComboboxList>
        </ComboboxContent>
      </Combobox>
    </div>
  )
}

/** Any other value, edited as JSON. */
function JsonField({
  id,
  value,
  label,
  onChange,
  onInvalidChange,
}: ArgumentFieldProps & { label: ReactNode }) {
  const [text, setText] = useState(() => formatJson(value))
  const [error, setError] = useState<string | null>(null)

  return (
    <Field data-invalid={error !== null}>
      {label}
      <Textarea
        id={id}
        value={text}
        className="font-mono"
        aria-invalid={error !== null}
        onChange={(event) => {
          setText(event.target.value)
          try {
            onChange(JSON.parse(event.target.value))
            setError(null)
            onInvalidChange(false)
          } catch {
            setError("Not valid JSON")
            onInvalidChange(true)
          }
        }}
      />
      {error && <FieldError>{error}</FieldError>}
    </Field>
  )
}
