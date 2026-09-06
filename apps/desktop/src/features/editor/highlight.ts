import { StateField } from '@codemirror/state'
import { Decoration, EditorView } from '@codemirror/view'

const keywords = new Set(
  'pub priv protected if else repeat while each in enum struct trait class let const mut static fn lambda async await return break continue use from export module try catch throw finally raise as'.split(
    ' ',
  ),
)
const types = new Set(
  'int long str decimal double char float short byte bool void'.split(' '),
)
const builtins = new Set(['print', 'to_string', 'round', 'scale'])

export function syntaxRanges(source: string) {
  // Match comments and literals first so their contents cannot become keywords.
  const tokens = [
    ...source.matchAll(
      /\/\/[^\r\n]*|\/\*[\s\S]*?(?:\*\/|$)|"(?:\\[^\r\n]|[^"\\\r\n])*(?:"|\\?(?=\r|\n|$))|'(?:\\[^\r\n]|[^'\\\r\n])*(?:'|\\?(?=\r|\n|$))|\d+(?:\.\d*)?(?:[eE][+-]?\d*)?(?:[\p{L}\p{N}_]*)|[\p{L}_][\p{L}\p{N}_]*|:=|->|\*\*|==|!=|<=|>=|[^\s]/gu,
    ),
  ]
  const significant = tokens.filter(
    ([text]) => !text.startsWith('//') && !text.startsWith('/*'),
  )
  const declaredTypes = new Set(
    significant.flatMap(([text], index) =>
      ['class', 'trait', 'struct', 'enum'].includes(
        significant[index - 1]?.[0] ?? '',
      )
        ? [text]
        : [],
    ),
  )
  const ranges: { from: number; to: number; class: string }[] = []
  let index = 0
  for (const token of tokens) {
    const text = token[0]
    const comment = text.startsWith('//') || text.startsWith('/*')
    const previous = significant[index - 1]?.[0]
    const next = significant[index + 1]?.[0]
    if (!comment) index += 1
    let kind = 'identifier'
    if (comment) kind = 'comment'
    else if (/^["']/.test(text)) kind = 'string'
    else if (/^\d/.test(text) || text === 'true' || text === 'false')
      kind = 'number'
    else if (
      types.has(text) ||
      declaredTypes.has(text) ||
      (/^[\p{L}_]/u.test(text) && (previous === ':' || previous === '->'))
    )
      kind = 'type'
    else if (keywords.has(text)) kind = 'keyword'
    else if (
      text === 'this' ||
      (builtins.has(text) && previous !== '.' && next === '(')
    )
      kind = 'builtin'
    else if (
      /^[\p{L}_]/u.test(text) &&
      !['and', 'or', 'not'].includes(text) &&
      (previous === 'fn' || next === '(')
    )
      kind = 'function'
    else if (/^(?:and|or|not|:=|->|[+*/%=!<>-]+)$/.test(text)) kind = 'operator'
    else if (/^[{}()[\],.;:]$/.test(text)) kind = 'punctuation'
    else if (!/^[\p{L}_][\p{L}\p{N}_]*$/u.test(text)) kind = 'error'
    ranges.push({
      from: token.index,
      to: token.index + text.length,
      class: `itd-token-${kind}`,
    })
    if (kind !== 'string') continue
    for (const escape of text.matchAll(/\\[^\r\n]/g)) {
      const from = token.index + escape.index
      ranges.push({
        from,
        to: from + escape[0].length,
        class: 'itd-token-escape',
      })
    }
  }
  return ranges
}

const decorate = (source: string) =>
  Decoration.set(
    syntaxRanges(source).map((range) =>
      Decoration.mark({ class: range.class }).range(range.from, range.to),
    ),
    true,
  )

// ponytail: full-document scan per edit; use incremental parsing if large files cause typing latency.
export const syntaxHighlighting = StateField.define({
  create: (state) => decorate(state.doc.toString()),
  update: (value, transaction) =>
    transaction.docChanged ? decorate(transaction.newDoc.toString()) : value,
  provide: (field) => EditorView.decorations.from(field),
})

// Official palette and syntax roles: https://catppuccin.com/palette/ and
// https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md
export const catppuccinMocha = EditorView.theme(
  {
    '&': { backgroundColor: '#1e1e2e', color: '#cdd6f4', colorScheme: 'dark' },
    '.cm-content': { caretColor: '#f5e0dc' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#f5e0dc' },
    '.cm-gutters': {
      backgroundColor: '#181825',
      color: '#9399b2',
      borderColor: '#313244',
    },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: '#313244' },
    '.cm-activeLineGutter': { color: '#cdd6f4' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, .cm-content ::selection':
      { backgroundColor: '#9399b240' },
    '.cm-selectionMatch': { backgroundColor: '#9399b233' },
    '.cm-matchingBracket': { backgroundColor: '#45475a', color: '#f5e0dc' },
    '.cm-tooltip, .cm-panels': {
      backgroundColor: '#181825',
      color: '#cdd6f4',
      borderColor: '#45475a',
    },
    '.cm-searchMatch': { backgroundColor: '#f9e2af33' },
    '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: '#fab38755' },
    '.cm-diagnostic-error': { borderLeftColor: '#f38ba8' },
    '.cm-diagnostic-warning': { borderLeftColor: '#f9e2af' },
    '.itd-token-identifier': { color: '#cdd6f4' },
    '.itd-token-keyword': { color: '#cba6f7' },
    '.itd-token-string': { color: '#a6e3a1' },
    '.itd-token-number': { color: '#fab387' },
    '.itd-token-type': { color: '#f9e2af' },
    '.itd-token-function': { color: '#89b4fa' },
    '.itd-token-operator': { color: '#89dceb' },
    '.itd-token-punctuation, .itd-token-comment': { color: '#9399b2' },
    '.itd-token-comment': { fontStyle: 'italic' },
    '.itd-token-builtin, .itd-token-error': { color: '#f38ba8' },
    '.itd-token-escape': { color: '#f5c2e7' },
  },
  { dark: true },
)
