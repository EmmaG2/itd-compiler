<script lang="ts">
  import type { NativeInspectionDto, LocationDto } from '$lib/types/compiler'
  import strings from '../../locales/es.json'

  let {
    inspection,
    onSelect,
  }: {
    inspection: NativeInspectionDto | null
    onSelect: (location: LocationDto) => void
  } = $props()
</script>

<div class="h-full overflow-auto">
  {#if inspection?.quadruples.length}
    <table class="w-full border-collapse font-mono text-xs">
      <thead class="sticky top-0 bg-card text-left text-muted-foreground">
        <tr class="border-b">
          <th class="p-2 font-medium">{strings.quadrupleOperation}</th>
          <th class="p-2 font-medium">{strings.quadrupleArg1}</th>
          <th class="p-2 font-medium">{strings.quadrupleArg2}</th>
          <th class="p-2 font-medium">{strings.quadrupleResult}</th>
          <th class="p-2 font-medium">{strings.quadrupleContext}</th>
        </tr>
      </thead>
      <tbody>
        {#each inspection.quadruples as row}
          <tr class="border-b hover:bg-accent">
            <td class="p-0">
              <button
                class="w-full p-2 text-left focus-visible:outline-2 focus-visible:outline-ring"
                onclick={() => onSelect(row.location)}
                title={`${row.location.file}:${row.location.line}:${row.location.column}`}
                >{row.operation}</button
              >
            </td>
            <td class="break-all p-2">{row.arg1}</td>
            <td class="break-all p-2">{row.arg2}</td>
            <td class="break-all p-2">{row.result}</td>
            <td class="break-all p-2 text-muted-foreground"
              >{row.function}{row.valueType ? ` · ${row.valueType}` : ''}</td
            >
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">{strings.noQuadruples}</p>
  {/if}
  {#if inspection?.truncated}<p class="notice">{strings.quadruplesTruncated}</p>{/if}
  {#if inspection?.diagnostics.length}
    <div class="border-t p-3 text-sm text-destructive">
      {#each inspection.diagnostics as diagnostic}
        <button class="block text-left underline" onclick={() => onSelect(diagnostic.location)}
          >{diagnostic.code}: {diagnostic.message}</button
        >
      {/each}
    </div>
  {/if}
  {#if inspection?.assembly}
    <section class="border-t p-3" aria-label={strings.arm64Assembly}>
      <h3 class="mb-2 text-sm font-medium">{strings.arm64Assembly}</h3>
      <pre class="overflow-auto rounded-md border bg-background p-3 font-mono text-xs leading-relaxed">{inspection.assembly}</pre>
    </section>
  {/if}
</div>
