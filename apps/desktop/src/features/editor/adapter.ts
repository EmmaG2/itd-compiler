import type { Diagnostic as CodeMirrorDiagnostic } from '@codemirror/lint'
import type { DiagnosticDto, LocationDto, TokenDto } from '$lib/types/compiler'

export const boundedRange = (location: LocationDto, length: number) => {
  const from = Math.min(Math.max(location.start, 0), length)
  return { from, to: Math.max(from, Math.min(location.end, length)) }
}

export const codeMirrorDiagnostics = (
  items: DiagnosticDto[],
  length: number,
): CodeMirrorDiagnostic[] =>
  items.map((item) => ({
    ...boundedRange(item.location, length),
    severity: item.severity,
    message: `${item.code}: ${item.message}`,
  }))

export const tokenClass = (kind: string) => `itd-token-${kind.toLowerCase()}`
export const tokenRanges = (items: TokenDto[], length: number) =>
  items
    .filter((item) => item.kind !== 'Eof')
    .map((item) => ({
      ...boundedRange(item.location, length),
      class: tokenClass(item.kind),
    }))
