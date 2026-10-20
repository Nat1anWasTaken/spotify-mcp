import { TOOL_CALL_STATUSES } from "@/components/tool-call-status-badge"
import { Field, FieldLabel } from "@/components/ui/field"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useSessions } from "@/hooks/use-sessions"
import { useTools } from "@/hooks/use-tools"
import type { ToolCallFilters } from "@/hooks/use-tool-calls"

// Base UI selects need a concrete value for the "no filter" option.
const ALL = "all"

type ActivityFiltersProps = {
  filters: ToolCallFilters
  onFilterChange: (
    key: keyof ToolCallFilters,
    value: string | undefined
  ) => void
}

export function ActivityFilters({
  filters,
  onFilterChange,
}: ActivityFiltersProps) {
  const sessions = useSessions()
  const tools = useTools()

  const sessionItems = [
    { value: ALL, label: "All sessions" },
    ...(sessions.data ?? []).map((session) => ({
      value: session.id,
      label: session.name,
    })),
  ]
  const statusItems = [
    { value: ALL, label: "All statuses" },
    ...Object.entries(TOOL_CALL_STATUSES).map(([value, { label }]) => ({
      value,
      label,
    })),
  ]

  const toolNames = (tools.data ?? []).map((tool) => tool.name).sort()
  // Keep a filter from the URL selectable even if the tool no longer exists.
  if (filters.tool && !toolNames.includes(filters.tool)) {
    toolNames.unshift(filters.tool)
  }
  const toolItems = [
    { value: ALL, label: "All tools" },
    ...toolNames.map((name) => ({ value: name, label: name })),
  ]

  return (
    <div className="flex flex-wrap gap-4">
      <FilterSelect
        id="filter-session"
        label="Session"
        items={sessionItems}
        value={filters.session_id}
        onValueChange={(value) => onFilterChange("session_id", value)}
      />
      <FilterSelect
        id="filter-status"
        label="Status"
        items={statusItems}
        value={filters.status}
        onValueChange={(value) => onFilterChange("status", value)}
      />
      <FilterSelect
        id="filter-tool"
        label="Tool"
        items={toolItems}
        value={filters.tool}
        onValueChange={(value) => onFilterChange("tool", value)}
      />
    </div>
  )
}

type FilterSelectProps = {
  id: string
  label: string
  items: { value: string; label: string }[]
  value: string | undefined
  onValueChange: (value: string | undefined) => void
}

function FilterSelect({
  id,
  label,
  items,
  value,
  onValueChange,
}: FilterSelectProps) {
  return (
    <Field className="w-56">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select
        items={items}
        value={value ?? ALL}
        onValueChange={(next) =>
          onValueChange(next === null || next === ALL ? undefined : next)
        }
      >
        <SelectTrigger id={id} className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {items.map((item) => (
            <SelectItem key={item.value} value={item.value}>
              {item.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </Field>
  )
}
