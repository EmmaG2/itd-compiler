<script lang="ts">
  import { Background, BackgroundVariant, Controls, MiniMap, SvelteFlow } from '@xyflow/svelte'
  import { Maximize2, X } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import AstNode from './AstNode.svelte'
  import { layoutAst } from './layout'
  import strings from '../../locales/es.json'
  import type { AstFlowNode } from './layout'
  import type { Edge, NodeTypes } from '@xyflow/svelte'
  import type { AstGraphDto, LocationDto } from '$lib/types/compiler'

  let { graph, onSelect, emptyLabel, truncatedLabel }: { graph: AstGraphDto; onSelect: (location: LocationDto) => void; emptyLabel: string; truncatedLabel: string } = $props()
  const nodeTypes = { ast: AstNode } satisfies NodeTypes
  const fitViewOptions = { padding: 0.2, minZoom: 0.03, maxZoom: 1 }
  const ariaLabelConfig = {
    'controls.zoomIn.ariaLabel': strings.zoomIn,
    'controls.zoomOut.ariaLabel': strings.zoomOut,
    'controls.fitView.ariaLabel': strings.fitView,
    'controls.interactive.ariaLabel': strings.toggleInteraction,
  }
  let nodes = $state.raw<AstFlowNode[]>([])
  let edges = $state.raw<Edge[]>([])
  let expanded = $state(false)
  let dialog: HTMLDialogElement
  $effect(() => { const layout = layoutAst($state.snapshot(graph)); nodes = layout.nodes; edges = layout.edges })
  function expand() { expanded = true; dialog.showModal() }
  function select(location: LocationDto | null) {
    if (!location) return
    if (expanded) dialog.close()
    onSelect(location)
  }
</script>

{#snippet flow()}
  <div class="ast-flow">
    <SvelteFlow bind:nodes bind:edges {nodeTypes} colorMode="dark" fitView {fitViewOptions} minZoom={0.03} {ariaLabelConfig} onlyRenderVisibleElements nodesDraggable={false} nodesConnectable={false} elementsSelectable onnodeclick={({ node }) => select(node.data.node.location)}>
      <Controls showLock={false} {fitViewOptions} />
      <MiniMap ariaLabel={strings.minimap} pannable zoomable />
      <Background variant={BackgroundVariant.Dots} gap={20} size={1} />
    </SvelteFlow>
  </div>
{/snippet}

<div class="flex h-full min-h-0 flex-col">
  {#if graph.nodes.length}
    <div class="flex shrink-0 justify-end border-b p-2">
      <Button variant="outline" size="sm" onclick={expand} aria-haspopup="dialog"><Maximize2 />{strings.expandAst}</Button>
    </div>
    <div class="min-h-0 flex-1">{#if !expanded}{@render flow()}{/if}</div>
  {:else}<p class="empty">{emptyLabel}</p>{/if}
  {#if graph.truncated}<p class="notice shrink-0">{truncatedLabel}</p>{/if}
</div>

<dialog bind:this={dialog} class="ast-dialog" aria-labelledby="ast-dialog-title" aria-describedby="ast-dialog-help" onclose={() => { expanded = false }}>
  <header class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b pb-3">
    <div>
      <h2 id="ast-dialog-title" class="text-lg font-semibold">{strings.astFullscreen}</h2>
      <p id="ast-dialog-help" class="mt-1 text-sm text-muted-foreground">{strings.astHelp}</p>
    </div>
    <Button variant="outline" onclick={() => dialog.close()}><X />{strings.closeAst}</Button>
  </header>
  <div class="min-h-0 flex-1 overflow-hidden rounded-lg border">{#if expanded}{@render flow()}{/if}</div>
  {#if graph.truncated}<p class="notice shrink-0">{truncatedLabel}</p>{/if}
</dialog>
