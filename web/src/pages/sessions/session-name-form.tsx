import { useState, type FormEvent } from "react"

import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { useUpdateSession } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"

export function SessionNameForm({ session }: { session: AgentSession }) {
  const update = useUpdateSession(session.id)
  // Local draft: a failed save leaves the typed name in place to retry.
  const [name, setName] = useState(session.name)
  const trimmed = name.trim()

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    update.mutate({ name: trimmed })
  }

  return (
    <Card>
      <CardContent>
        <form onSubmit={handleSubmit}>
          <Field orientation="horizontal">
            <FieldLabel htmlFor="session-name">Name</FieldLabel>
            <Input
              id="session-name"
              required
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
            <Button
              type="submit"
              variant="outline"
              disabled={
                update.isPending || !trimmed || trimmed === session.name
              }
            >
              Save
            </Button>
          </Field>
        </form>
      </CardContent>
    </Card>
  )
}
