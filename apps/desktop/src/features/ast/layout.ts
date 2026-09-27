import strings from '../../locales/es.json'
import type { Edge, Node } from '@xyflow/svelte'
import type { AstGraphDto, AstNodeDto } from '$lib/types/compiler'

export type AstFlowNode = Node<{ node: AstNodeDto }, 'ast'>

export function semanticLabels(semantic: AstNodeDto['semantic']): string[] {
  if (!semantic) return []
  const labels = [
    `${strings.semanticType}: ${semantic.inferredType ?? strings.unresolvedType}`,
  ]
  if (semantic.sourceType)
    labels.push(
      `${strings.semanticConversion}: ${semantic.sourceType} → ${semantic.inferredType ?? strings.unresolvedType}`,
    )
  if (semantic.symbol)
    labels.push(
      `${strings.semanticSymbol}: ${semantic.symbol.name} (#${semantic.symbol.id})`,
    )
  return labels
}

export function layoutAst(graph: AstGraphDto): {
  nodes: AstFlowNode[]
  edges: Edge[]
} {
  const children = new Map<string, AstNodeDto[]>()
  for (const node of graph.nodes)
    if (node.parentId)
      children.set(node.parentId, [
        ...(children.get(node.parentId) ?? []),
        node,
      ])
  const levelHeight = graph.nodes.some((node) => node.semantic) ? 180 : 130
  let leaf = 0
  const positions = new Map<string, { x: number; y: number }>()
  const visit = (node: AstNodeDto, depth: number): number => {
    const descendants = children.get(node.id) ?? []
    const childXs = descendants.map((child) => visit(child, depth + 1))
    const x = childXs.length
      ? (childXs[0]! + childXs[childXs.length - 1]!) / 2
      : leaf++ * 210
    positions.set(node.id, { x, y: depth * levelHeight })
    return x
  }
  for (const root of graph.nodes.filter((node) => !node.parentId))
    visit(root, 0)
  return {
    nodes: graph.nodes.map((node) => ({
      id: node.id,
      type: 'ast',
      position: positions.get(node.id) ?? { x: 0, y: 0 },
      data: { node },
    })),
    edges: graph.nodes
      .filter((node) => node.parentId)
      .map((node) => ({
        id: `${node.parentId}-${node.id}`,
        source: node.parentId!,
        target: node.id,
        label: node.relation ?? undefined,
        type: 'straight',
      })),
  }
}
