import { describe, expect, it } from 'vitest'
import type { SemanticTraceDto, TraceEventDto } from '$lib/types/compiler'
import { replayTrace } from './replay'

const location = { file: 'main.itd', start: 0, end: 1, line: 1, column: 1 }
const attribute = (expression: number) => ({
  expression,
  typeLabel: 'int',
  location,
})
const event = (changes: Partial<TraceEventDto>): TraceEventDto => ({
  kind: 'push',
  expression: 0,
  location,
  rule: 'literal',
  depthBefore: 0,
  depthAfter: 1,
  popCount: 0,
  pushed: attribute(0),
  conversionType: null,
  ...changes,
})
const trace: SemanticTraceDto = {
  truncated: false,
  events: [
    event({}),
    event({
      expression: 1,
      depthBefore: 1,
      depthAfter: 2,
      pushed: attribute(1),
    }),
    event({
      kind: 'reduce',
      rule: 'add',
      expression: 2,
      depthBefore: 2,
      depthAfter: 1,
      popCount: 2,
      pushed: attribute(2),
    }),
    event({
      kind: 'conversion',
      expression: 2,
      depthBefore: 1,
      depthAfter: 1,
      pushed: null,
      conversionType: 'decimal',
    }),
    event({
      kind: 'consume',
      expression: 2,
      depthBefore: 1,
      depthAfter: 0,
      popCount: 1,
      pushed: null,
    }),
  ],
}

describe('semantic stack replay', () => {
  it('reconstructs the initial, intermediate and final states without mutating the trace', () => {
    expect(replayTrace(trace, 0)).toEqual({
      stack: [],
      operands: [],
      event: null,
    })
    const reduced = replayTrace(trace, 3)
    expect(reduced.stack).toEqual([attribute(2)])
    expect(reduced.operands).toEqual([attribute(0), attribute(1)])
    expect(replayTrace(trace, 4).stack).toEqual(reduced.stack)
    expect(replayTrace(trace, 5).stack).toEqual([])
    expect(replayTrace(trace, 1).stack).toEqual([attribute(0)])
    expect(trace.events[2]!.popCount).toBe(2)
  })
  it('shows real separately consumed operands on their reduction', () => {
    const events = [
      event({}),
      event({
        expression: 1,
        depthBefore: 1,
        depthAfter: 2,
        pushed: attribute(1),
      }),
      event({
        kind: 'consume',
        expression: 1,
        depthBefore: 2,
        depthAfter: 1,
        popCount: 1,
        pushed: null,
      }),
      event({
        kind: 'consume',
        expression: 0,
        depthBefore: 1,
        depthAfter: 0,
        popCount: 1,
        pushed: null,
      }),
      event({
        kind: 'conversion',
        expression: 0,
        depthBefore: 0,
        depthAfter: 0,
        pushed: null,
        conversionType: 'decimal',
      }),
      event({
        kind: 'reduce',
        expression: 2,
        depthBefore: 0,
        depthAfter: 1,
        pushed: attribute(2),
      }),
    ]
    const actual = { truncated: false, events }
    expect(replayTrace(actual, 3).operands).toEqual([attribute(1)])
    expect(replayTrace(actual, 6).operands).toEqual([
      attribute(0),
      attribute(1),
    ])
    expect(replayTrace(actual, 6).stack).toEqual([attribute(2)])
    const following = [
      ...events,
      event({
        kind: 'consume',
        expression: 2,
        depthBefore: 1,
        depthAfter: 0,
        popCount: 1,
        pushed: null,
      }),
      event({}),
      event({ kind: 'reduce', depthBefore: 1, depthAfter: 2 }),
    ]
    expect(
      replayTrace({ truncated: false, events: following }, following.length)
        .operands,
    ).toEqual([])
  })
  it('preserves a truncated prefix instead of pretending it ends empty', () => {
    expect(
      replayTrace({ truncated: true, events: trace.events.slice(0, 2) }, 2)
        .stack,
    ).toHaveLength(2)
  })
  it('replays recovery without removing an enclosing attribute', () => {
    const recovery = event({
      kind: 'recover',
      depthBefore: 2,
      depthAfter: 1,
      popCount: 1,
      pushed: null,
    })
    expect(
      replayTrace(
        { truncated: false, events: [...trace.events.slice(0, 2), recovery] },
        3,
      ).stack,
    ).toEqual([attribute(0)])
  })
  it('rejects invalid steps, underflow and inconsistent depths', () => {
    for (const step of [-1, 6, 0.5, NaN])
      expect(() => replayTrace(trace, step)).toThrow()
    for (const invalid of [
      event({ popCount: 1 }),
      event({ depthBefore: 2 }),
      event({ depthAfter: 4 }),
    ]) {
      expect(() =>
        replayTrace({ truncated: false, events: [invalid] }, 1),
      ).toThrow()
    }
  })
})
