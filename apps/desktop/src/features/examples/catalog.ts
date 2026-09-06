import manifest from '../../../../../docs/language/examples/catalog.json'
import { ErrorWithCode } from '$lib/errors'

const sources = import.meta.glob<string>(
  '../../../../../docs/language/examples/*.itd',
  { eager: true, query: '?raw', import: 'default' },
)
const outputs = import.meta.glob<string>(
  '../../../../../docs/language/examples/*.expected.txt',
  { eager: true, query: '?raw', import: 'default' },
)
export const examples = manifest.map((item) => {
  const base = `../../../../../docs/language/examples/${item.id}`
  const source = sources[`${base}.itd`]
  const expected = outputs[`${base}.expected.txt`]
  if (source === undefined || expected === undefined)
    throw new ErrorWithCode('INVALID_CATALOG', item.id)
  return { ...item, source, expected, path: `${item.id}.itd` }
})
