<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Badge } from '$lib/components/ui/badge'
  import { Label } from '$lib/components/ui/label'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import * as Card from '$lib/components/ui/card'
  import * as Tabs from '$lib/components/ui/tabs'
  import * as Table from '$lib/components/ui/table'
  import { Play, Square, ScanLine, Terminal, FileCode } from '@lucide/svelte'
  import Editor from './features/editor/Editor.svelte'
  import AstGraph from './features/ast/AstGraph.svelte'
  import BytecodeView from './features/bytecode/BytecodeView.svelte'
  import NativeView from './features/bytecode/NativeView.svelte'
  import SemanticStack from './features/semantic-stack/SemanticStack.svelte'
  import { examples } from './features/examples/catalog'
  import {
    analyzeSource,
    cancelRun,
    inspectNative,
    runSource,
    commandErrorMessage,
  } from '$lib/ipc/compiler'
  import strings from './locales/es.json'
  import type {
    AnalysisDto,
    BytecodeDto,
    DiagnosticDto,
    LocationDto,
    NativeInspectionDto,
    SymbolDto,
    TokenDto,
  } from '$lib/types/compiler'

  type Status = 'ready' | 'analyzing' | 'running' | 'cancelled' | 'failed'
  let source = $state(examples[0]!.source)
  let loadedSource = $state(examples[0]!.source)
  let exampleId = $state(examples[0]!.id)
  let selectedExample = $derived(
    examples.find((example) => example.id === exampleId)!,
  )
  let replaceDialog: HTMLDialogElement
  let bytecode = $state<BytecodeDto>({ rows: [], truncated: false })
  let nativeInspection = $state<NativeInspectionDto | null>(null)
  let nativeInspectionRevision = $state<number | null>(null)
  let nativeBusy = $state(false)
  let virtualPath = $state('main.itd')
  let status = $state<Status>('ready')
  let diagnostics = $state<DiagnosticDto[]>([])
  let tokens = $state<TokenDto[]>([])
  let symbols = $state<SymbolDto[]>([])
  let ast = $state<AnalysisDto['ast']>({ nodes: [], truncated: false })
  let semanticAst = $state<AnalysisDto['semanticAst']>(null)
  let semanticStack = $state<AnalysisDto['semanticStack']>(null)
  let treeView = $state('syntax')
  const emptyGraph: AnalysisDto['ast'] = { nodes: [], truncated: false }
  let output = $state(strings.noOutput)
  let activeTab = $state('diagnostics')
  let reveal = $state<LocationDto | null>(null)
  let executionId = $state<string | null>(null)
  let revision = $state(0)
  let busy = $derived(
    status === 'analyzing' ||
      status === 'running' ||
      executionId !== null ||
      nativeBusy,
  )

  function invalidate(value: string) {
    source = value
    revision += 1
    diagnostics = []
    tokens = []
    symbols = []
    ast = { nodes: [], truncated: false }
    semanticAst = null
    semanticStack = null
    bytecode = { rows: [], truncated: false }
    nativeInspectionRevision = null
    reveal = null
    output = strings.noOutput
    if (!busy) setStatus('ready')
  }
  function setStatus(value: Status) {
    status = value
  }
  function showDiagnostics(values: DiagnosticDto[]) {
    diagnostics = values
    activeTab = 'diagnostics'
  }
  function navigate(location: LocationDto) {
    reveal = { ...location }
  }
  function loadExample() {
    if (busy) return
    if (source !== loadedSource) {
      replaceDialog.showModal()
      return
    }
    replaceExample()
  }
  function replaceExample() {
    replaceDialog.close()
    loadedSource = selectedExample.source
    virtualPath = selectedExample.path
    invalidate(selectedExample.source)
  }
  function downloadCode() {
    const url = URL.createObjectURL(
      new Blob([source], { type: 'text/plain;charset=utf-8' }),
    )
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = virtualPath.split(/[\\/]/).pop() || 'main.itd'
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 1000)
  }

  async function analyze() {
    if (busy) return
    const requestRevision = revision
    setStatus('analyzing')
    try {
      const result = await analyzeSource(source, virtualPath)
      if (requestRevision !== revision) {
        setStatus('ready')
        return
      }
      diagnostics = result.diagnostics
      tokens = result.tokens
      symbols = result.symbols
      ast = result.ast
      semanticAst = result.semanticAst
      semanticStack = result.semanticStack
      bytecode = result.bytecode
      setStatus(
        result.diagnostics.some((item) => item.severity === 'error')
          ? 'failed'
          : 'ready',
      )
    } catch (error: unknown) {
      if (requestRevision !== revision) {
        setStatus('ready')
        return
      }
      fail(error)
    }
  }
  async function generateNative() {
    if (busy) return
    const requestRevision = revision
    const requestPath = virtualPath
    nativeBusy = true
    try {
      const result = await inspectNative(source, requestPath)
      if (requestRevision !== revision || requestPath !== virtualPath) return
      nativeInspection = result
      nativeInspectionRevision = requestRevision
      activeTab = 'native'
    } catch (error: unknown) {
      if (requestRevision !== revision || requestPath !== virtualPath) return
      nativeInspection = {
        quadruples: [],
        truncated: false,
        assembly: null,
        diagnostics: [
          {
            code: 'NATIVE_INSPECTION',
            severity: 'error',
            message: commandErrorMessage(error),
            location: {
              file: requestPath,
              start: 0,
              end: 0,
              line: 1,
              column: 1,
            },
          },
        ],
      }
      nativeInspectionRevision = requestRevision
    } finally {
      nativeBusy = false
    }
  }
  async function run() {
    if (busy) return
    const requestRevision = revision
    const requestPath = virtualPath
    executionId = crypto.randomUUID()
    setStatus('running')
    output = strings.noOutput
    diagnostics = []
    bytecode = { rows: [], truncated: false }
    try {
      const result = await runSource(source, requestPath, executionId)
      if (requestRevision !== revision || requestPath !== virtualPath) {
        setStatus('ready')
        return
      }
      bytecode = result.bytecode
      output = result.output || strings.noOutput
      showDiagnostics(result.diagnostics)
      setStatus(
        result.diagnostics.some((item) => item.code === 'E5004')
          ? 'cancelled'
          : result.diagnostics.some((item) => item.severity === 'error')
            ? 'failed'
            : 'ready',
      )
    } catch (error: unknown) {
      if (requestRevision === revision) fail(error)
    } finally {
      executionId = null
      if (requestRevision !== revision) setStatus('ready')
    }
  }
  async function stop() {
    if (!executionId) return
    try {
      await cancelRun(executionId)
      setStatus('cancelled')
    } catch (error: unknown) {
      fail(error)
    }
  }
  function fail(error: unknown) {
    output = commandErrorMessage(error)
    setStatus('failed')
  }
</script>

<svelte:head><title>{strings.title}</title></svelte:head>
<div class="mx-auto flex min-h-screen max-w-[1600px] flex-col gap-6 p-4 sm:p-6">
  <Card.Root size="sm">
    <Card.Content
      class="flex flex-wrap items-end gap-4"
      aria-label={strings.actions}
    >
      <div class="grid min-w-40 flex-1 gap-2">
        <Label for="virtual-path">{strings.virtualPath}</Label>
        <Input
          id="virtual-path"
          bind:value={virtualPath}
          disabled={busy}
          oninput={() => invalidate(source)}
        />
      </div>
      <div class="flex flex-wrap gap-2">
        <Button onclick={analyze} disabled={busy}
          ><ScanLine />{strings.analyze}</Button
        >
        <Button variant="secondary" onclick={run} disabled={busy}
          ><Play />{strings.run}</Button
        >
        <Button variant="outline" onclick={generateNative} disabled={busy}
          >{strings.generateArm64}</Button
        >
        <Button variant="outline" onclick={stop} disabled={!executionId}
          ><Square />{strings.stop}</Button
        >
      </div>
    </Card.Content>
  </Card.Root>

  <section class="rounded-xl border bg-card p-4" aria-label={strings.examples}>
    <div class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="example">{strings.examples}</Label>
        <select
          id="example"
          class="h-9 rounded-md border bg-background px-3 text-sm"
          bind:value={exampleId}
          disabled={busy}
        >
          {#each examples as example}<option value={example.id}
              >{example.title}</option
            >{/each}
        </select>
      </div>
      <Button variant="outline" onclick={loadExample} disabled={busy}
        >{strings.loadExample}</Button
      >
    </div>
    <p class="mt-3 text-sm text-muted-foreground">
      {selectedExample.description}
    </p>
    <details class="mt-2 text-sm">
      <summary class="cursor-pointer">{strings.expectedOutput}</summary>
      <pre
        class="mt-2 whitespace-pre-wrap font-mono">{selectedExample.expected}</pre>
    </details>
  </section>

  <dialog
    bind:this={replaceDialog}
    class="m-auto max-w-lg rounded-xl border bg-card p-6 text-card-foreground backdrop:bg-black/60"
    aria-labelledby="replace-title"
  >
    <h2 id="replace-title" class="text-lg font-semibold">
      {strings.loadExample}
    </h2>
    <p class="my-4">{strings.replacePrompt}</p>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" onclick={() => replaceDialog.close()}
        >{strings.keepChanges}</Button
      >
      <Button variant="outline" onclick={downloadCode}
        >{strings.downloadCode}</Button
      >
      <Button onclick={replaceExample}>{strings.replaceCode}</Button>
    </div>
  </dialog>

  <main class="grid min-w-0 gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
    <Card.Root class="min-w-0">
      <Card.Header>
        <Card.Title
          ><h2 class="flex items-center gap-2">
            <FileCode class="size-4 text-muted-foreground" />{strings.source}
          </h2></Card.Title
        >
        <Card.Action><Badge variant="outline">{virtualPath}</Badge></Card.Action
        >
      </Card.Header>
      <Card.Content>
        <div
          class="h-[min(60vh,640px)] min-h-80 overflow-hidden rounded-md border"
        >
          <Editor
            bind:source
            {diagnostics}
            onChange={invalidate}
            {reveal}
            label={strings.source}
          />
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root class="min-w-0">
      <Card.Header>
        <Card.Title><h2>{strings.analysisResults}</h2></Card.Title>
        <Card.Action
          ><Badge variant="secondary">{tokens.length} {strings.tokens}</Badge
          ></Card.Action
        >
      </Card.Header>
      <Card.Content>
        <Tabs.Root bind:value={activeTab} class="gap-4">
          <Tabs.List
            class="w-full flex-wrap"
            style="height: auto; min-height: 2.25rem"
            aria-label={strings.analysisResults}
          >
            <Tabs.Trigger value="diagnostics"
              >{strings.diagnostics}</Tabs.Trigger
            >
            <Tabs.Trigger value="symbols">{strings.symbols}</Tabs.Trigger>
            <Tabs.Trigger value="ast">{strings.ast}</Tabs.Trigger>
            <Tabs.Trigger value="semantic-stack"
              >{strings.semanticStack}</Tabs.Trigger
            >
            <Tabs.Trigger value="bytecode">{strings.bytecode}</Tabs.Trigger>
            <Tabs.Trigger value="native">{strings.nativeInspection}</Tabs.Trigger>
          </Tabs.List>
          <Tabs.Content value="semantic-stack">
            {#key semanticStack}
              <SemanticStack trace={semanticStack} onSelect={navigate} />
            {/key}
          </Tabs.Content>
          <Tabs.Content value="diagnostics">
            <ScrollArea class="h-[max(280px,calc(min(60vh,640px)-52px))]">
              {#if diagnostics.length}
                <ol class="space-y-2">
                  {#each diagnostics as diagnostic}
                    <li>
                      <Button
                        variant="ghost"
                        class="h-auto w-full justify-start whitespace-normal p-3 text-left"
                        onclick={() => navigate(diagnostic.location)}
                      >
                        <span class="grid min-w-0 gap-2">
                          <span class="flex flex-wrap items-center gap-2">
                            <Badge
                              variant={diagnostic.severity === 'error'
                                ? 'destructive'
                                : 'secondary'}>{diagnostic.code}</Badge
                            >
                            <span
                              class="break-all font-mono text-xs text-muted-foreground"
                              >{diagnostic.location.file}:{diagnostic.location
                                .line}:{diagnostic.location.column}</span
                            >
                          </span>
                          <span class="break-words font-normal leading-relaxed"
                            >{diagnostic.message}</span
                          >
                        </span>
                      </Button>
                    </li>
                  {/each}
                </ol>
              {:else}
                <p class="py-12 text-center text-sm text-muted-foreground">
                  {strings.noDiagnostics}
                </p>
              {/if}
            </ScrollArea>
          </Tabs.Content>
          <Tabs.Content value="symbols">
            <ScrollArea
              class="h-[max(280px,calc(min(60vh,640px)-52px))]"
              orientation="both"
            >
              {#if symbols.length}
                <Table.Root>
                  <Table.Header
                    ><Table.Row
                      ><Table.Head>{strings.symbolName}</Table.Head><Table.Head
                        >{strings.symbolKind}</Table.Head
                      ><Table.Head>{strings.symbolType}</Table.Head></Table.Row
                    ></Table.Header
                  >
                  <Table.Body
                    >{#each symbols as symbol}<Table.Row
                        ><Table.Cell class="font-mono">{symbol.name}</Table.Cell
                        ><Table.Cell>{symbol.kind}</Table.Cell><Table.Cell
                          class="font-mono">{symbol.ty}</Table.Cell
                        ></Table.Row
                      >{/each}</Table.Body
                  >
                </Table.Root>
              {:else}<p class="py-12 text-center text-sm text-muted-foreground">
                  {strings.noSymbols}
                </p>{/if}
            </ScrollArea>
          </Tabs.Content>
          <Tabs.Content
            value="ast"
            class="flex-none h-[max(360px,calc(min(60vh,640px)-52px))] overflow-hidden rounded-md border"
          >
            <Tabs.Root
              bind:value={treeView}
              class="flex h-full min-h-0 flex-col gap-0"
            >
              <Tabs.List
                aria-label={strings.treeViews}
                class="m-2 shrink-0 self-start"
              >
                <Tabs.Trigger value="syntax">{strings.syntaxTree}</Tabs.Trigger>
                <Tabs.Trigger value="semantic"
                  >{strings.semanticTree}</Tabs.Trigger
                >
              </Tabs.List>
              <Tabs.Content value="syntax" class="min-h-0 flex-1">
                {#if activeTab === 'ast' && treeView === 'syntax'}
                  <AstGraph
                    graph={ast}
                    onSelect={navigate}
                    emptyLabel={strings.noAst}
                    truncatedLabel={strings.astTruncated}
                    title={strings.syntaxTitle}
                    expandLabel={strings.expandSyntax}
                  />
                {/if}
              </Tabs.Content>
              <Tabs.Content value="semantic" class="min-h-0 flex-1">
                {#if activeTab === 'ast' && treeView === 'semantic'}
                  <div class="flex h-full min-h-0 flex-col">
                    <p class="px-3 pb-2 text-sm text-muted-foreground">
                      {strings.semanticHelp}
                    </p>
                    <div class="min-h-0 flex-1">
                      <AstGraph
                        graph={semanticAst ?? emptyGraph}
                        onSelect={navigate}
                        emptyLabel={strings.noSemanticAst}
                        truncatedLabel={strings.semanticTruncated}
                        title={strings.semanticTitle}
                        expandLabel={strings.expandSemantic}
                        help={strings.semanticHelp + ' ' + strings.astHelp}
                      />
                    </div>
                  </div>
                {/if}
              </Tabs.Content>
            </Tabs.Root>
          </Tabs.Content>
          <Tabs.Content
            value="bytecode"
            class="h-[max(360px,calc(min(60vh,640px)-52px))] overflow-hidden rounded-md border"
          >
            <BytecodeView {bytecode} onSelect={navigate} />
          </Tabs.Content>
          <Tabs.Content
            value="native"
            class="h-[max(360px,calc(min(60vh,640px)-52px))] overflow-hidden rounded-md border"
          >
            <NativeView
              inspection={nativeInspectionRevision === revision
                ? nativeInspection
                : null}
              onSelect={navigate}
            />
          </Tabs.Content>
        </Tabs.Root>
      </Card.Content>
    </Card.Root>

    <Card.Root class="min-w-0 lg:col-span-2">
      <Card.Header
        ><Card.Title
          ><h2 class="flex items-center gap-2">
            <Terminal class="size-4 text-muted-foreground" />{strings.output}
          </h2></Card.Title
        ></Card.Header
      >
      <Card.Content>
        <ScrollArea class="h-40 rounded-md border bg-background">
          <pre
            class="whitespace-pre-wrap break-words p-4 font-mono text-sm leading-relaxed"
            aria-live="polite">{output}</pre>
        </ScrollArea>
      </Card.Content>
    </Card.Root>
  </main>
</div>
