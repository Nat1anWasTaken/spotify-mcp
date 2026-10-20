import { keepPreviousData, useQuery } from "@tanstack/react-query"

import { useDebouncedValue } from "@/hooks/use-debounced-value"
import { api, type FormattedText, type ParseMode } from "@/lib/api"

/** `text` formatted by the server exactly as sending it would. */
export function useFormattedText(text: string, parseMode: ParseMode) {
  const debouncedText = useDebouncedValue(text, 300)
  return useQuery({
    queryKey: ["format-preview", parseMode, debouncedText],
    queryFn: () =>
      api.post<FormattedText>("/api/format-preview", {
        text: debouncedText,
        parse_mode: parseMode,
      }),
    placeholderData: keepPreviousData,
    staleTime: Infinity,
  })
}
