<script lang="ts">
  import type { BytecodeDto, LocationDto } from '$lib/types/compiler'
  import strings from '../../locales/es.json'
  let { bytecode, onSelect }: { bytecode: BytecodeDto; onSelect: (location: LocationDto) => void } = $props()
</script>
<div class="h-full overflow-auto">
  {#if bytecode.rows.length}
    <ol class="divide-y divide-border font-mono text-xs">
      {#each bytecode.rows as row}
        <li>
          <button class="grid w-full grid-cols-[minmax(0,1fr)_3rem_minmax(0,2fr)] gap-2 p-2 text-left hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring" onclick={() => onSelect(row.location)} title={`${row.location.file}:${row.location.line}:${row.location.column}`}>
            <span class="truncate text-muted-foreground">{row.function}</span>
            <span class="text-muted-foreground">{row.offset}</span>
            <span class="break-all">{row.instruction}</span>
          </button>
        </li>
      {/each}
    </ol>
  {:else}<p class="empty">{strings.noBytecode}</p>{/if}
  {#if bytecode.truncated}<p class="notice">{strings.bytecodeTruncated}</p>{/if}
</div>
