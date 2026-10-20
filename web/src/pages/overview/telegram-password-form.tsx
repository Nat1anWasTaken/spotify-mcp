import { useState, type FormEvent } from "react"

import { Button } from "@/components/ui/button"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { useSubmitTelegramPassword } from "@/hooks/use-telegram"

export function TelegramPasswordForm({ hint }: { hint: string | null }) {
  const [password, setPassword] = useState("")
  const submit = useSubmitTelegramPassword()

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    submit.mutate(password)
  }

  return (
    <form onSubmit={handleSubmit} className="flex max-w-sm flex-col gap-4">
      <Field data-invalid={submit.isError || undefined}>
        <FieldLabel htmlFor="telegram-password">
          Two-step verification password
        </FieldLabel>
        <Input
          id="telegram-password"
          type="password"
          autoComplete="current-password"
          required
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          aria-invalid={submit.isError || undefined}
        />
        {hint && <FieldDescription>Hint: {hint}</FieldDescription>}
        {submit.isError && <FieldError>{submit.error.message}</FieldError>}
      </Field>
      <Button type="submit" className="self-start" disabled={submit.isPending}>
        {submit.isPending && <Spinner />}
        Continue
      </Button>
    </form>
  )
}
