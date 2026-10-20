import { useState } from "react"

import {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  ComboboxValue,
  useComboboxAnchor,
} from "@/components/ui/combobox"
import { useDebouncedValue } from "@/hooks/use-debounced-value"
import { usePeers } from "@/hooks/use-peers"
import type { PeerSummary } from "@/lib/api"

type PeerPickerProps = {
  id: string
  value: PeerSummary[]
  onValueChange: (peers: PeerSummary[]) => void
}

/** Multi-select of Telegram chats, searched server-side via `/api/peers`. */
export function PeerPicker({ id, value, onValueChange }: PeerPickerProps) {
  const anchor = useComboboxAnchor()
  const [query, setQuery] = useState("")
  const peers = usePeers(useDebouncedValue(query, 250))

  return (
    <Combobox
      multiple
      items={peers.data ?? []}
      filter={null}
      value={value}
      onValueChange={onValueChange}
      inputValue={query}
      onInputValueChange={setQuery}
      itemToStringLabel={(peer) => peer.title}
      isItemEqualToValue={(item, selected) => item.id === selected.id}
    >
      <ComboboxChips ref={anchor}>
        <ComboboxValue>
          {(selected: PeerSummary[]) => (
            <>
              {selected.map((peer) => (
                <ComboboxChip key={peer.id}>{peer.title}</ComboboxChip>
              ))}
              <ComboboxChipsInput
                id={id}
                placeholder={selected.length ? "" : "Search chats…"}
              />
            </>
          )}
        </ComboboxValue>
      </ComboboxChips>
      <ComboboxContent anchor={anchor}>
        <ComboboxEmpty>
          {peers.isError ? "Couldn't load chats." : "No chats found."}
        </ComboboxEmpty>
        <ComboboxList>
          {(peer: PeerSummary) => (
            <ComboboxItem key={peer.id} value={peer}>
              {peer.title}
              {peer.username && (
                <span className="text-muted-foreground">@{peer.username}</span>
              )}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  )
}
