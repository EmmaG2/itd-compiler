import { expect, it } from 'vitest'
import { examples } from './catalog'

it('loads every documented example and expected output from the shared files', () => {
  expect(examples).toHaveLength(10)
  expect(new Set(examples.map((example) => example.id)).size).toBe(10)
  for (const example of examples) {
    expect(example.source.length).toBeGreaterThan(0)
    expect(example.expected.endsWith('\n')).toBe(true)
  }
  expect(
    examples.find((example) => example.id === 'expressions')?.expected,
  ).toBe('0.6\n0.16\n0.09\n')
})
