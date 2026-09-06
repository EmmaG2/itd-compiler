import { describe, expect, it } from 'vitest'
import { layoutAst } from './layout'

describe('layoutAst', () => {
  it('keeps an empty graph empty and lays out siblings without overlap', () => {
    expect(layoutAst({ nodes: [], truncated: false }).nodes).toEqual([])
    const graph = {
      truncated: false,
      nodes: [
        {
          id: 'ast-0',
          parentId: null,
          relation: null,
          kind: 'Program',
          label: 'Program',
          location: null,
        },
        {
          id: 'ast-1',
          parentId: 'ast-0',
          relation: 'left',
          kind: 'Name',
          label: 'a',
          location: null,
        },
        {
          id: 'ast-2',
          parentId: 'ast-0',
          relation: 'right',
          kind: 'Name',
          label: 'b',
          location: null,
        },
      ],
    }
    const { nodes, edges } = layoutAst(graph)
    expect(nodes.map((node) => node.id)).toEqual(['ast-0', 'ast-1', 'ast-2'])
    expect(nodes[0]!.position.y).toBeLessThan(nodes[1]!.position.y)
    expect(nodes[1]!.position.x).not.toBe(nodes[2]!.position.x)
    expect(edges).toHaveLength(2)
  })
})
