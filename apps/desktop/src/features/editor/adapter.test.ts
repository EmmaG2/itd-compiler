import { describe, expect, it } from 'vitest'
import { boundedRange, codeMirrorDiagnostics, tokenRanges } from './adapter'

const location = { file: 'main.itd', start: -2, end: 12, line: 1, column: 1 }

describe('editor adapter', () => {
  it('bounds compiler ranges and preserves severity', () => {
    expect(boundedRange(location, 5)).toEqual({ from: 0, to: 5 })
    expect(
      codeMirrorDiagnostics(
        [{ code: 'E1', severity: 'error', message: 'bad', location }],
        5,
      )[0],
    ).toMatchObject({ from: 0, to: 5, severity: 'error' })
    expect(
      tokenRanges(
        [
          { kind: 'Let', lexeme: 'let', location },
          { kind: 'Eof', lexeme: '', location },
        ],
        5,
      ),
    ).toHaveLength(1)
  })
})
