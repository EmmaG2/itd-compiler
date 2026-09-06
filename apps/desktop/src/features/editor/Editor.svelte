<script lang="ts">
  import { onMount } from 'svelte'
  import { basicSetup } from 'codemirror'
  import { EditorState, Annotation } from '@codemirror/state'
  import { EditorView, keymap } from '@codemirror/view'
  import { lintGutter, setDiagnostics } from '@codemirror/lint'
  import { codeMirrorDiagnostics } from './adapter'
  import { catppuccinMocha, syntaxHighlighting } from './highlight'
  import type { DiagnosticDto, LocationDto } from '$lib/types/compiler'

  let { source = $bindable(), diagnostics = [], onChange, reveal, label }: {
    source: string; diagnostics?: DiagnosticDto[]; onChange: (value: string) => void; reveal?: LocationDto | null; label: string
  } = $props()

  let host: HTMLDivElement
  let view: EditorView | undefined
  const externalChange = Annotation.define<boolean>()

  onMount(() => {
    view = new EditorView({
      state: EditorState.create({ doc: source, extensions: [basicSetup, lintGutter(), catppuccinMocha, syntaxHighlighting, EditorView.updateListener.of((update) => { if (update.docChanged && !update.transactions.some((transaction) => transaction.annotation(externalChange))) onChange(update.state.doc.toString()) }), keymap.of([])] }),
      parent: host,
    })
    updateMarkers()
    return () => view?.destroy()
  })

  function updateMarkers() {
    if (!view) return
    const length = view.state.doc.length
    view.dispatch(setDiagnostics(view.state, codeMirrorDiagnostics(diagnostics, length)))
  }

  $effect(() => {
    const value = source
    if (view && view.state.doc.toString() !== value) view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value }, annotations: externalChange.of(true) })
  })
  $effect(() => { diagnostics; updateMarkers() })
  $effect(() => {
    if (!view || !reveal) return
    const { from, to } = codeMirrorDiagnostics([{ code: '', severity: 'warning', message: '', location: reveal }], view.state.doc.length)[0]!
    view.dispatch({ selection: { anchor: from, head: to }, effects: EditorView.scrollIntoView(from, { y: 'center' }) })
    view.focus()
  })
</script>

<div class="editor" bind:this={host} aria-label={label}></div>
