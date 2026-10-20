import { ErrorAlert } from "@/components/error-alert"
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
  FieldLegend,
  FieldSet,
} from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUpdateSession } from "@/hooks/use-sessions"
import { useTools } from "@/hooks/use-tools"
import type { AgentSession, ToolInfo } from "@/lib/api"

const TOOL_GROUPS: { kind: ToolInfo["kind"]; legend: string }[] = [
  { kind: "read", legend: "Read tools" },
  { kind: "write", legend: "Write tools" },
]

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

  function setAllTools(all: boolean) {
    // Turning "All tools" off starts from an explicit list of every tool.
    update.mutate({
      allowed_tools: all ? null : toolList.map((tool) => tool.name),
    })
  }

  function setToolAllowed(name: string, isAllowed: boolean) {
    const current = allowed ?? []
    update.mutate({
      allowed_tools: isAllowed
        ? [...current, name]
        : current.filter((tool) => tool !== name),
    })
  }

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
      {TOOL_GROUPS.map(({ kind, legend }) => (
        <FieldSet key={kind}>
          <FieldLegend variant="label">{legend}</FieldLegend>
          {kind === "write" && session.read_only && (
            <FieldDescription>
              Read-only is on, so write tools are blocked anyway.
            </FieldDescription>
          )}
          <FieldGroup>
            {toolList
              .filter((tool) => tool.kind === kind)
              .map((tool) => {
                const id = `tool-${tool.name}`
                return (
                  <Field key={tool.name} orientation="horizontal">
                    <Checkbox
                      id={id}
                      checked={allowed === null || allowed.includes(tool.name)}
                      disabled={allowed === null}
                      onCheckedChange={(checked) =>
                        setToolAllowed(tool.name, checked)
                      }
                    />
                    <FieldContent>
                      <FieldLabel htmlFor={id}>{tool.title}</FieldLabel>
                      <FieldDescription>{tool.description}</FieldDescription>
                    </FieldContent>
                  </Field>
                )
              })}
          </FieldGroup>
        </FieldSet>
      ))}
    </FieldGroup>
  )
}
