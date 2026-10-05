<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import type { LocationDto, SemanticTraceDto } from '$lib/types/compiler'
  import strings from '../../locales/es.json'
  import { replayTrace } from './replay'

  let {
    trace,
    onSelect,
  }: {
    trace: SemanticTraceDto | null
    onSelect: (location: LocationDto) => void
  } = $props()
  let step = $state(0)
  let replay = $derived.by(() => {
    if (!trace) return null
    try {
      return replayTrace(trace, step)
    } catch {
      return null
    }
  })
  const kindLabels = {
    push: strings.stackPush,
    reduce: strings.stackReduce,
    consume: strings.stackConsume,
    conversion: strings.stackConversion,
    recover: strings.stackRecover,
  }
</script>

<section aria-label={strings.semanticStack} class="space-y-4">
  <p class="text-sm text-muted-foreground">{strings.stackHelp}</p>
  {#if !trace}
    <p role="status" class="py-8 text-sm text-muted-foreground">
      {strings.noSemanticStack}
    </p>
  {:else if !replay}
    <p role="alert">{strings.stackInvalid}</p>
  {:else}
    <div
      class="flex flex-wrap items-center gap-2"
      aria-label={strings.stackControls}
    >
      <Button
        variant="outline"
        size="sm"
        disabled={step === 0}
        onclick={() => (step = 0)}>{strings.stackReset}</Button
      >
      <Button
        variant="outline"
        size="sm"
        disabled={step === 0}
        onclick={() => (step -= 1)}>{strings.stackPrevious}</Button
      >
      <span class="font-mono text-sm" role="status" aria-live="polite"
        >{strings.stackStep} {step} / {trace.events.length}</span
      >
      <Button
        variant="outline"
        size="sm"
        disabled={step === trace.events.length}
        onclick={() => (step += 1)}>{strings.stackNext}</Button
      >
      <Button
        variant="outline"
        size="sm"
        disabled={step === trace.events.length}
        onclick={() => (step = trace.events.length)}>{strings.stackEnd}</Button
      >
    </div>
    {#if replay.event}
      <div class="space-y-2 border-l-2 border-primary pl-3 text-sm">
        <p>
          <strong>{kindLabels[replay.event.kind]}</strong> ·
          <code>{replay.event.rule}</code>
        </p>
        <p>
          {strings.stackDepth}: {replay.event.depthBefore} → {replay.event
            .depthAfter}
        </p>
        {#if replay.operands.length}
          <p>
            {strings.stackOperands}:
            <code
              >{replay.operands
                .map((item) => `${item.typeLabel} (#${item.expression})`)
                .join(', ')}</code
            >
          </p>
        {/if}
        {#if replay.event.pushed}
          <p>
            {strings.stackResult}:
            <code
              >{replay.event.pushed.typeLabel} (#{replay.event.pushed
                .expression})</code
            >
          </p>
        {/if}
        {#if replay.event.conversionType}
          <p>
            {strings.stackConversion}:
            <code
              >#{replay.event.expression} → {replay.event.conversionType}</code
            >
          </p>
        {/if}
        <Button
          variant="ghost"
          size="sm"
          onclick={() => onSelect(replay!.event!.location)}
          >{strings.stackSource} · {replay.event.location.line}:{replay.event
            .location.column}</Button
        >
      </div>
    {:else}<p class="text-sm text-muted-foreground">
        {strings.stackInitial}
      </p>{/if}
    <h3 class="text-sm font-semibold">
      {strings.stackCurrent} ({replay.stack.length})
    </h3>
    <div class="max-h-64 overflow-auto rounded-md border">
      {#if replay.stack.length}
        <ol aria-label={strings.stackCurrent} class="divide-y">
          {#each [...replay.stack].reverse() as item, index}
            <li
              class="flex items-center justify-between gap-2 px-3 py-2 font-mono text-sm"
            >
              <span
                >{replay.stack.length - index - 1} · {item.typeLabel} · #{item.expression}</span
              >
              <Button
                variant="ghost"
                size="sm"
                onclick={() => onSelect(item.location)}
                aria-label={`${strings.stackSource} #${item.expression}`}
                >{item.location.line}:{item.location.column}</Button
              >
            </li>
          {/each}
        </ol>
      {:else}<p class="p-3 text-sm text-muted-foreground">
          {strings.stackEmpty}
        </p>{/if}
    </div>
    {#if trace.truncated}<p role="status" class="text-sm text-muted-foreground">
        {strings.stackTruncated}
      </p>{/if}
    {#if replay.event?.pushed?.typeLabel === 'error' || replay.event?.kind === 'recover'}
      <p role="status" class="text-sm text-destructive">{strings.stackError}</p>
    {/if}
  {/if}
</section>
