import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Switch } from "@/components/ui/switch"
import { useUpdateSession, type SessionPatch } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"
import { OutboxTtlField } from "@/pages/sessions/outbox-ttl-field"

type PermissionKey = keyof Pick<
  SessionPatch,
  "enabled" | "read_only" | "use_outbox"
>

const PERMISSIONS: {
  key: PermissionKey
  label: string
  description: string
}[] = [
  {
    key: "enabled",
    label: "Enabled",
    description: "When off, the agent can't call any tool.",
  },
  {
    key: "read_only",
    label: "Read-only",
    description:
      "Blocks every write tool (sending, editing, deleting, …), whatever the settings below say.",
  },
  {
    key: "use_outbox",
    label: "Use the outbox",
    description:
      "Write tools go to your outbox, where you approve or discard them.",
  },
]

export function SessionPermissionsCard({ session }: { session: AgentSession }) {
  const update = useUpdateSession(session.id)

  return (
    <Card>
      <CardHeader>
        <CardTitle>Permissions</CardTitle>
        <CardDescription>Changes apply to the next tool call.</CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          {PERMISSIONS.map(({ key, label, description }) => {
            const id = `permission-${key}`
            return (
              <Field key={key} orientation="horizontal">
                <FieldContent>
                  <FieldLabel htmlFor={id}>{label}</FieldLabel>
                  <FieldDescription>{description}</FieldDescription>
                </FieldContent>
                <Switch
                  id={id}
                  checked={session[key]}
                  onCheckedChange={(checked) =>
                    update.mutate({ [key]: checked })
                  }
                />
              </Field>
            )
          })}
          <OutboxTtlField session={session} />
        </FieldGroup>
      </CardContent>
    </Card>
  )
}
