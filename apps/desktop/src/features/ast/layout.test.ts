import { describe, expect, it } from 'vitest'
import { layoutAst, semanticLabels } from './layout'

describe('layoutAst', () => {
  it('renders native semantic metadata without inventing types or symbols', () => {
    expect(semanticLabels(undefined)).toEqual([])
    expect(
      semanticLabels({ inferredType: null, sourceType: null, symbol: null }),
    ).toEqual(['Tipo: no resuelto'])
    expect(
      semanticLabels({ inferredType: 'Error', sourceType: null, symbol: null }),
    ).toEqual(['Tipo: Error'])
    expect(
      semanticLabels({
        inferredType: 'Decimal',
        sourceType: 'Int',
        symbol: { id: 7, name: 'count' },
      }),
    ).toEqual([
      'Tipo: Decimal',
      'Conversión: Int → Decimal',
      'Símbolo: count (#7)',
    ])
  })

  it('preserves semantic metadata and leaves room for annotated nodes', () => {
    const semantic = {
      inferredType: 'Int',
      sourceType: null,
      symbol: { id: 7, name: 'count' },
    }
    const { nodes } = layoutAst({
      truncated: false,
      nodes: [
        {
          id: 'root',
          parentId: null,
          relation: null,
          kind: 'Program',
          label: 'Program',
          location: null,
        },
        {
          id: 'name',
          parentId: 'root',
          relation: 'item',
          kind: 'Name',
          label: 'count',
          location: null,
          semantic,
        },
      ],
    })
    expect(nodes[1]!.data.node.semantic).toEqual(semantic)
    expect(nodes[1]!.position.y - nodes[0]!.position.y).toBeGreaterThanOrEqual(
      180,
    )
  })

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

  it('draws each repeated item relation on a distinct parent-child line in both trees', () => {
    const nodes = [
      {
        id: 'root',
        parentId: null,
        relation: null,
        kind: 'Program',
        label: 'Program',
        location: null,
      },
      ...['a', 'b', 'c', 'call'].map((id) => ({
        id,
        parentId: 'root',
        relation: 'item',
        kind: 'Variable',
        label: id,
        location: null,
      })),
    ]
    for (const semantic of [false, true]) {
      const graph = {
        truncated: false,
        nodes: nodes.map((node) =>
          semantic
            ? {
                ...node,
                semantic: {
                  inferredType: 'Void',
                  sourceType: null,
                  symbol: null,
                },
              }
            : node,
        ),
      }
      const { edges } = layoutAst(graph)
      expect(
        edges.map(({ source, target, label, type }) => ({
          source,
          target,
          label,
          type,
        })),
      ).toEqual(
        ['a', 'b', 'c', 'call'].map((target) => ({
          source: 'root',
          target,
          label: 'item',
          type: 'straight',
        })),
      )
    }
  })
})
