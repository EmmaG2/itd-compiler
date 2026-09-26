<script lang="ts">
  import { semanticLabels } from './layout'
  import { Handle, Position } from '@xyflow/svelte'
  import type { Node, NodeProps } from '@xyflow/svelte'
  import type { AstNodeDto } from '$lib/types/compiler'
  type AstNode = Node<{ node: AstNodeDto }, 'ast'>
  let { data }: NodeProps<AstNode> = $props()
</script>

<Handle type="target" position={Position.Top} />
<article class="ast-node">
  <strong>{data.node.kind}</strong>
  <span>{data.node.label}</span>
  {#each semanticLabels(data.node.semantic) as label}
    <span title={label}>{label}</span>
  {/each}
</article>
<Handle type="source" position={Position.Bottom} />
