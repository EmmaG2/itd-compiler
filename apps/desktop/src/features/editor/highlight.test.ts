import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { syntaxHighlighting, syntaxRanges } from './highlight'

const colored = (source: string) =>
  syntaxRanges(source).map((range) => [
    source.slice(range.from, range.to),
    range.class.replace('itd-token-', ''),
  ])

describe('ITD syntax highlighting', () => {
  it('distinguishes language roles, including functions across comments', () => {
    expect(
      colored(
        'pub fn área /* note */ (x: decimal) -> bool { return x >= 1.2e-3dec and true; }',
      ),
    ).toEqual([
      ['pub', 'keyword'],
      ['fn', 'keyword'],
      ['área', 'function'],
      ['/* note */', 'comment'],
      ['(', 'punctuation'],
      ['x', 'identifier'],
      [':', 'punctuation'],
      ['decimal', 'type'],
      [')', 'punctuation'],
      ['->', 'operator'],
      ['bool', 'type'],
      ['{', 'punctuation'],
      ['return', 'keyword'],
      ['x', 'identifier'],
      ['>=', 'operator'],
      ['1.2e-3dec', 'number'],
      ['and', 'operator'],
      ['true', 'number'],
      [';', 'punctuation'],
      ['}', 'punctuation'],
    ])
    expect(
      colored(
        'class Shape {} let s: Shape := Shape(); s.area(); print(to_string(round(1dec, 2)));',
      ),
    ).toContainEqual(['Shape', 'type'])
    expect(colored('scale(1dec); this.area();')).toEqual([
      ['scale', 'builtin'],
      ['(', 'punctuation'],
      ['1dec', 'number'],
      [')', 'punctuation'],
      [';', 'punctuation'],
      ['this', 'builtin'],
      ['.', 'punctuation'],
      ['area', 'function'],
      ['(', 'punctuation'],
      [')', 'punctuation'],
      [';', 'punctuation'],
    ])
  })

  it('isolates comments, literals and escapes with UTF-16 offsets', () => {
    const source = '"😀 // if \\n" // true\r\n/* fn */ \'a\' let café := 2;'
    expect(colored(source)).toEqual([
      ['"😀 // if \\n"', 'string'],
      ['\\n', 'escape'],
      ['// true', 'comment'],
      ['/* fn */', 'comment'],
      ["'a'", 'string'],
      ['let', 'keyword'],
      ['café', 'identifier'],
      [':=', 'operator'],
      ['2', 'number'],
      [';', 'punctuation'],
    ])
    expect(
      syntaxRanges(source).find(
        (range) => source.slice(range.from, range.to) === 'café',
      )?.from,
    ).toBe(source.indexOf('café'))
    expect(colored('"unfinished\nlet n := 1;')).toContainEqual([
      'let',
      'keyword',
    ])
    expect(colored('/* unfinished\nlet n := 1;')).toEqual([
      ['/* unfinished\nlet n := 1;', 'comment'],
    ])
  })

  it('colors the initial document and recomputes after edits without compiler tokens', () => {
    const state = EditorState.create({
      doc: 'let n := 1;',
      extensions: [syntaxHighlighting],
    })
    expect(state.field(syntaxHighlighting).size).toBe(5)
    const updated = state.update({
      changes: { from: 0, to: state.doc.length, insert: '// let n := 1;' },
    }).state
    const decorations = updated.field(syntaxHighlighting)
    expect(decorations.size).toBe(1)
    expect(decorations.iter().value?.spec.class).toBe('itd-token-comment')
    expect(
      updated
        .update({ selection: { anchor: 2 } })
        .state.field(syntaxHighlighting),
    ).toBe(decorations)
  })
})
