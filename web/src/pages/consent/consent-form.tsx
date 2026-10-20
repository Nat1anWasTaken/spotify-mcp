import { useState, type FormEvent } from "react"
import { TriangleAlertIcon } from "lucide-react"

import { ErrorAlert } from "@/components/error-alert"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
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
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { useSignedInMe } from "@/hooks/use-me"
import { useDecideOAuthRequest } from "@/hooks/use-oauth-request"
import type { OAuthRequest } from "@/lib/api"

type ConsentFormProps = {
  requestId: string
  request: OAuthRequest
}

export function ConsentForm({ requestId, request }: ConsentFormProps) {
  const { telegram } = useSignedInMe()
  const decide = useDecideOAuthRequest(requestId)
  const clientName = request.client_name ?? request.origin ?? "An AI agent"
  const existing = request.existing_session
  const [name, setName] = useState(existing?.name ?? clientName)
  const [readOnly, setReadOnly] = useState(existing?.read_only ?? true)
  const [useOutbox, setUseOutbox] = useState(existing?.use_outbox ?? true)

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    decide.mutate({
      approve: true,
      name: name.trim() || clientName,
      read_only: readOnly,
      use_outbox: useOutbox,
    })
  }

  // Stay busy after success too: the browser is navigating to the client.
  const busy = decide.isPending || decide.isSuccess
  const approving = decide.variables?.approve

  return (
    <form onSubmit={handleSubmit}>
      <Card>
        <CardHeader>
          <Avatar size="lg">
            {request.logo_uri && <AvatarImage src={request.logo_uri} alt="" />}
            <AvatarFallback>
              {clientName.slice(0, 1).toUpperCase()}
            </AvatarFallback>
          </Avatar>
          <CardTitle>{clientName} wants to use your Telegram account</CardTitle>
          <CardDescription>
            <ClientDetails request={request} />
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          {!telegram.linked && (
            <Alert>
              <TriangleAlertIcon />
              <AlertTitle>Telegram isn't linked yet</AlertTitle>
              <AlertDescription>
                You can approve now, but the agent won't be able to do anything
                until you link Telegram on the dashboard.
              </AlertDescription>
            </Alert>
          )}
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="consent-name">Session name</FieldLabel>
              <Input
                id="consent-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
              <FieldDescription>
                How this connection appears on the dashboard.
              </FieldDescription>
            </Field>
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="consent-read-only">Read-only</FieldLabel>
                <FieldDescription>
                  Blocks every write tool. You can change this later.
                </FieldDescription>
              </FieldContent>
              <Switch
                id="consent-read-only"
                checked={readOnly}
                onCheckedChange={setReadOnly}
              />
            </Field>
            <Field orientation="horizontal">
              <FieldContent>
                <FieldLabel htmlFor="consent-use-outbox">
                  Use the outbox
                </FieldLabel>
                <FieldDescription>
                  Write tools go to your outbox on the dashboard, where you
                  approve or discard them.
                </FieldDescription>
              </FieldContent>
              <Switch
                id="consent-use-outbox"
                checked={useOutbox}
                onCheckedChange={setUseOutbox}
              />
            </Field>
          </FieldGroup>
          {decide.isError && <ErrorAlert error={decide.error} />}
        </CardContent>
        <CardFooter className="justify-end gap-2">
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => decide.mutate({ approve: false })}
          >
            {busy && approving === false && <Spinner />}
            Deny
          </Button>
          <Button type="submit" disabled={busy}>
            {busy && approving === true && <Spinner />}
            Allow
          </Button>
        </CardFooter>
      </Card>
    </form>
  )
}

function ClientDetails({ request }: { request: OAuthRequest }) {
  const redirectHost =
    URL.parse(request.redirect_uri)?.host || request.redirect_uri

  return (
    <span className="flex flex-col gap-1">
      {request.origin && <span>From {request.origin}</span>}
      {request.client_uri && (
        <a href={request.client_uri} target="_blank" rel="noreferrer">
          {request.client_uri}
        </a>
      )}
      <span>
        After you decide, you'll be sent to <strong>{redirectHost}</strong>.
      </span>
    </span>
  )
}
