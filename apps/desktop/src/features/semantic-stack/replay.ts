import { ErrorWithCode } from '$lib/errors'
import type { SemanticTraceDto, TraceAttributeDto } from '$lib/types/compiler'

export function replayTrace(trace: SemanticTraceDto, step: number) {
  if (!Number.isInteger(step) || step < 0 || step > trace.events.length)
    throw new ErrorWithCode('INVALID_TRACE_STEP', 'Invalid semantic trace step')
  const stack: TraceAttributeDto[] = []
  let operands: TraceAttributeDto[] = []
  let consumed: TraceAttributeDto[] = []
  for (const event of trace.events.slice(0, step)) {
    if (
      event.depthBefore !== stack.length ||
      !Number.isInteger(event.popCount) ||
      event.popCount < 0 ||
      event.popCount > stack.length
    )
      throw new ErrorWithCode('INVALID_TRACE', 'Invalid semantic stack delta')
    operands = stack.splice(stack.length - event.popCount, event.popCount)
    if (event.kind === 'consume') consumed.unshift(...operands)
    if (event.kind === 'reduce') {
      // Core consumption events precede reduction; do not pop their values twice.
      operands = [...operands, ...consumed]
      consumed = []
    }
    if (event.kind === 'push' || event.kind === 'recover') consumed = []
    if (event.pushed) stack.push(event.pushed)
    if (event.depthAfter !== stack.length)
      throw new ErrorWithCode('INVALID_TRACE', 'Invalid semantic stack depth')
  }
  return { stack, operands, event: trace.events[step - 1] ?? null }
}
