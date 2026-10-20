import { ErrorAlert } from "@/components/error-alert"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldSet,
} from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUpdateSession } from "@/hooks/use-sessions"
import { useTools } from "@/hooks/use-tools"
import type { AgentSession, ToolGroup, ToolInfo } from "@/lib/api"

/** Display order and labels for `ToolInfo.group`. */
const TOOL_GROUPS: Record<ToolGroup, string> = {
  player: "Playback",
  search: "Search",
  catalog: "Catalog",
  playlists: "Playlists",
  library: "Library",
  follow: "Follows",
  personalization: "Top items & history",
  users: "Profiles",
}

export function SessionToolsCard({ session }: { session: AgentSession }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Tools</CardTitle>
        <CardDescription>Which tools this agent may call.</CardDescription>
      </CardHeader>
      <CardContent>
        <SessionToolsContent session={session} />
      </CardContent>
    </Card>
  )
}

function SessionToolsContent({ session }: { session: AgentSession }) {
  const tools = useTools()
  const update = useUpdateSession(session.id)
  const allowed = session.allowed_tools

  if (tools.isPending) {
    return <Skeleton className="h-40 w-full" />
  }
  if (tools.isError) {
    return <ErrorAlert title="Couldn't load tools" error={tools.error} />
  }
  const toolList = tools.data
  const isAllowed = (name: string) => allowed === null || allowed.includes(name)

  function setAllTools(all: boolean) {
    // Turning "All tools" off starts from an explicit list of every tool.
    update.mutate({
      allowed_tools: all ? null : toolList.map((tool) => tool.name),
    })
  }

  function setToolsAllowed(names: string[], value: boolean) {
    const current = allowed ?? []
    update.mutate({
      allowed_tools: value
        ? [...new Set([...current, ...names])]
        : current.filter((tool) => !names.includes(tool)),
    })
  }

  // Groups the server knows but this build doesn't go last, under their key.
  const groups = [
    ...new Set([
      ...(Object.keys(TOOL_GROUPS) as ToolGroup[]),
      ...toolList.map((tool) => tool.group),
    ]),
  ]
    .map((group) => ({
      group,
      tools: toolList.filter((tool) => tool.group === group),
    }))
    .filter(({ tools }) => tools.length > 0)

  return (
    <FieldGroup>
      <Field orientation="horizontal">
        <FieldContent>
          <FieldLabel htmlFor="all-tools">All tools</FieldLabel>
          <FieldDescription>
            Includes tools added in future versions.
          </FieldDescription>
        </FieldContent>
        <Switch
          id="all-tools"
          checked={allowed === null}
          onCheckedChange={setAllTools}
        />
      </Field>
      {session.read_only && (
        <FieldDescription>
          Read-only is on, so write tools are blocked whatever is checked here.
        </FieldDescription>
      )}
      {groups.map(({ group, tools: groupTools }) => (
        <ToolGroupFieldSet
          key={group}
          group={group}
          tools={groupTools}
          isAllowed={isAllowed}
          editable={allowed !== null}
          readOnly={session.read_only}
          onChange={setToolsAllowed}
        />
      ))}
    </FieldGroup>
  )
}

type ToolGroupFieldSetProps = {
  group: ToolGroup
  tools: ToolInfo[]
  isAllowed: (name: string) => boolean
  editable: boolean
  readOnly: boolean
  onChange: (names: string[], allowed: boolean) => void
}

function ToolGroupFieldSet({
  group,
  tools,
  isAllowed,
  editable,
  readOnly,
  onChange,
}: ToolGroupFieldSetProps) {
  const names = tools.map((tool) => tool.name)
  const allowedCount = names.filter(isAllowed).length
  const groupId = `tool-group-${group}`

  return (
    <FieldSet aria-label={TOOL_GROUPS[group] ?? group}>
      <Field orientation="horizontal">
        <Checkbox
          id={groupId}
          checked={allowedCount === names.length}
          disabled={!editable}
          onCheckedChange={(checked) => onChange(names, checked)}
        />
        <FieldLabel htmlFor={groupId} className="font-semibold">
          {TOOL_GROUPS[group] ?? group}
        </FieldLabel>
        <span className="ml-auto text-xs text-muted-foreground tabular-nums">
          {allowedCount}/{names.length}
        </span>
      </Field>
      <FieldGroup className="gap-4 pl-6">
        {tools.map((tool) => {
          const id = `tool-${tool.name}`
          const blocked = readOnly && tool.kind === "write"
          return (
            <Field key={tool.name} orientation="horizontal">
              <Checkbox
                id={id}
                checked={isAllowed(tool.name)}
                disabled={!editable}
                onCheckedChange={(checked) => onChange([tool.name], checked)}
              />
              <FieldContent>
                <FieldLabel
                  htmlFor={id}
                  className={blocked ? "text-muted-foreground" : undefined}
                >
                  {tool.title}
                  <Badge
                    variant={tool.kind === "write" ? "outline" : "secondary"}
                  >
                    {tool.kind === "write" ? "Write" : "Read"}
                  </Badge>
                  {tool.destructive && (
                    <Badge variant="destructive">Destructive</Badge>
                  )}
                </FieldLabel>
                <FieldDescription className="line-clamp-2">
                  <code className="text-xs">{tool.name}</code> ·{" "}
                  {tool.description}
                </FieldDescription>
              </FieldContent>
            </Field>
          )
        })}
      </FieldGroup>
    </FieldSet>
  )
}
