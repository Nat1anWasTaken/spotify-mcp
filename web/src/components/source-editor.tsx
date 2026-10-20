import { defaultKeymap, history, historyKeymap } from "@codemirror/commands"
import { html } from "@codemirror/lang-html"
import { markdown } from "@codemirror/lang-markdown"
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language"
import { Compartment, EditorState, type Extension } from "@codemirror/state"
import {
  EditorView,
  keymap,
  placeholder as placeholderText,
} from "@codemirror/view"
import { tags } from "@lezer/highlight"
import { cn } from "cn"
import { useEffect, useRef } from "react"

import type { ParseMode } from "@/lib/api"

/** Colours markup only; the text itself stays plain so it reads as written. */
const highlightStyle = HighlightStyle.define([
  { tag: tags.heading, color: "var(--syntax-heading)", fontWeight: "600" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.monospace, color: "var(--syntax-code)" },
  { tag: tags.link, color: "var(--link)" },
  { tag: tags.url, color: "var(--link)", textDecoration: "underline" },
  { tag: tags.quote, color: "var(--muted-foreground)" },
  // Markdown's *, `, [ ]( ) and # marks.
  { tag: tags.processingInstruction, color: "var(--syntax-mark)" },
  { tag: tags.contentSeparator, color: "var(--syntax-mark)" },
  { tag: tags.list, color: "var(--syntax-mark)" },
  // HTML.
  { tag: tags.tagName, color: "var(--syntax-mark)" },
  { tag: tags.angleBracket, color: "var(--syntax-mark)" },
  { tag: tags.attributeName, color: "var(--syntax-heading)" },
  { tag: tags.attributeValue, color: "var(--link)" },
  { tag: tags.comment, color: "var(--muted-foreground)" },
])

const theme = EditorView.theme({
  "&": {
    color: "var(--foreground)",
    backgroundColor: "transparent",
    fontSize: "0.875rem",
    height: "100%",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    lineHeight: "1.6",
  },
  ".cm-content": { padding: "0.5rem 0", caretColor: "var(--foreground)" },
  ".cm-line": { padding: "0 0.75rem" },
  ".cm-cursor": { borderLeftColor: "var(--foreground)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection":
    { backgroundColor: "color-mix(in oklch, var(--ring) 35%, transparent)" },
  ".cm-placeholder": { color: "var(--muted-foreground)" },
})

function languageExtension(language: ParseMode): Extension {
  switch (language) {
    case "markdown":
      // CommonMark, as the server parses it.
      return markdown()
    case "html":
      return html({ autoCloseTags: false })
    case "plain":
      return []
  }
}

type SourceEditorProps = {
  id?: string
  value: string
  onChange: (value: string) => void
  /** The markup being written. */
  language: ParseMode
  placeholder?: string
  className?: string
  "aria-invalid"?: boolean
}

/** A plain-text editor for markup you type yourself, with highlighting. */
export function SourceEditor({
  id,
  value,
  onChange,
  language,
  placeholder,
  className,
  "aria-invalid": ariaInvalid,
}: SourceEditorProps) {
  const parent = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)
  const languageSlot = useRef(new Compartment())
  const onChangeRef = useRef(onChange)
  useEffect(() => {
    onChangeRef.current = onChange
  })

  // The view is created once; later props are pushed into it below.
  useEffect(() => {
    const created = new EditorView({
      parent: parent.current!,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          EditorView.lineWrapping,
          syntaxHighlighting(highlightStyle),
          theme,
          placeholder ? placeholderText(placeholder) : [],
          EditorView.contentAttributes.of(id ? { id } : {}),
          languageSlot.current.of(languageExtension(language)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current(update.state.doc.toString())
            }
          }),
        ],
      }),
    })
    view.current = created
    return () => {
      created.destroy()
      view.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Changes from outside, such as undoing every edit.
  useEffect(() => {
    const current = view.current
    if (current && current.state.doc.toString() !== value) {
      current.dispatch({
        changes: { from: 0, to: current.state.doc.length, insert: value },
      })
    }
  }, [value])

  useEffect(() => {
    view.current?.dispatch({
      effects: languageSlot.current.reconfigure(languageExtension(language)),
    })
  }, [language])

  return (
    <div
      ref={parent}
      aria-invalid={ariaInvalid}
      className={cn(
        "min-h-32 overflow-hidden rounded-lg border border-input bg-transparent transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 aria-invalid:border-destructive dark:bg-input/30",
        className
      )}
    />
  )
}
