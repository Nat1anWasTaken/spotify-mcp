import { useState, type FormEvent } from "react"

import { Button } from "@/components/ui/button"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
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
import { useUpdateSession } from "@/hooks/use-sessions"
import type { AgentSession } from "@/lib/api"

/** The window the server accepts: a minute to a week. */
const MIN_MINUTES = 1
const MAX_MINUTES = 7 * 24 * 60

const UNITS = [
  { value: "minutes", label: "minutes", minutes: 1 },
  { value: "hours", label: "hours", minutes: 60 },
  { value: "days", label: "days", minutes: 24 * 60 },
] as const

type Unit = (typeof UNITS)[number]["value"]

const unitMinutes = (unit: Unit) =>
  UNITS.find((item) => item.value === unit)!.minutes

/** The largest unit that shows `minutes` as a whole number. */
function split(minutes: number): { amount: number; unit: Unit } {
  const unit = [...UNITS].reverse().find((item) => minutes % item.minutes === 0)
  const chosen = unit ?? UNITS[0]
  return { amount: minutes / chosen.minutes, unit: chosen.value }
}

export function OutboxTtlField({ session }: { session: AgentSession }) {
  const update = useUpdateSession(session.id)
  // Local draft: a failed save leaves the typed value in place to retry.
  const initial = split(session.outbox_ttl_minutes)
  const [amount, setAmount] = useState(String(initial.amount))
  const [unit, setUnit] = useState<Unit>(initial.unit)

  const parsed = Number(amount)
  const minutes = Number.isInteger(parsed) ? parsed * unitMinutes(unit) : NaN
  const valid = minutes >= MIN_MINUTES && minutes <= MAX_MINUTES

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (valid) {
      update.mutate({ outbox_ttl_minutes: minutes })
    }
  }

  return (
    <form onSubmit={handleSubmit}>
      <Field orientation="horizontal" data-invalid={!valid || undefined}>
        <FieldContent>
          <FieldLabel htmlFor="outbox-ttl">Outbox expiration</FieldLabel>
          <FieldDescription>
            How long a write action waits in your outbox before it expires, from
            1 minute to 1 week. Applies to actions queued from now on.
          </FieldDescription>
          {!valid && (
            <FieldError>
              Pick a whole number from 1 minute to 1 week.
            </FieldError>
          )}
        </FieldContent>
        <div className="flex shrink-0 items-center gap-2">
          <Input
            id="outbox-ttl"
            type="number"
            inputMode="numeric"
            min={1}
            step={1}
            className="w-20"
            aria-invalid={!valid || undefined}
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
          />
          <Select
            items={UNITS}
            value={unit}
            onValueChange={(next) => next && setUnit(next)}
          >
            <SelectTrigger aria-label="Unit" className="w-28">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {UNITS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            type="submit"
            variant="outline"
            disabled={
              update.isPending ||
              !valid ||
              minutes === session.outbox_ttl_minutes
            }
          >
            Save
          </Button>
        </div>
      </Field>
    </form>
  )
}
