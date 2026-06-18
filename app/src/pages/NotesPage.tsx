import type { JSX } from 'solid-js'
import { createEffect, createMemo, createResource, createSignal, For, Show } from 'solid-js'
import {
  BookOpen,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  Coins,
  ExternalLink,
  FilePlus2,
  Fish,
  Folder,
  FolderPlus,
  Hammer,
  Info,
  Leaf,
  MoreHorizontal,
  PackageSearch,
  Pencil,
  Plus,
  Search,
  Shuffle,
  Trash2,
  X,
  RotateCcw,
  ZoomIn,
  ZoomOut,
} from 'lucide-solid'
import {
  buildCraftTree,
  collapseKey,
  CRAFT_TYPE_ABBRS,
  CRAFT_TYPE_NAMES,
  craftableRecipes,
  createCraftDataEngine,
  cx,
  defaultSourceIndex,
  formatInteger,
  getIconUrls,
  getItem,
  getItemName,
  loadCraftData,
  resolveSource,
  sourceLabel,
  sourcePriority,
  summarizeMaterials,
  type CraftDataEngine,
  type CraftDataPackage,
  type CraftItem,
  type CraftRecipe,
  type CraftTreeNode,
  type ItemSource,
  type MaterialSummary,
  type SourceChoice,
} from '../lib'
import { Badge, Button, EmptyState, Input } from '../ui'

const NOTES_STORAGE_KEY = 'xiv-companion-notes-v1'
const MARKET_WORLD_DC_REGION = '中国'
const UNIVERSALIS_BASE_URL = import.meta.env.DEV ? '/api/universalis' : 'https://universalis.app'
const GRAPH_EDGE_COLOR = '#78716c'
const CRYSTAL_ELEMENTS = ['火', '冰', '风', '土', '雷', '水'] as const
const CRYSTAL_TIERS = ['碎晶', '水晶', '晶簇'] as const
const CRYSTAL_ELEMENT_COLORS = {
  火: '#b91c1c',
  冰: '#0891b2',
  风: '#15803d',
  土: '#a16207',
  雷: '#7c3aed',
  水: '#0f766e',
} satisfies Record<CrystalElement, string>

type CrystalElement = typeof CRYSTAL_ELEMENTS[number]
type CrystalTier = typeof CRYSTAL_TIERS[number]
type CrystalResource = {
  element: CrystalElement
  tier: CrystalTier
}

type NoteTreeNode = {
  id: string
  kind: 'folder' | 'page'
  title: string
  children?: NoteTreeNode[]
}

type CraftSummaryTarget = {
  id: string
  recipeId: number
  itemId: number
  amount: number
  collapsed: string[]
}

type CraftSummaryCard = {
  id: string
  kind: 'craftSummary'
  title: string
  targets: CraftSummaryTarget[]
  sourceChoices?: Record<string, SourceChoice>
}

type NotePage = {
  id: string
  cards: CraftSummaryCard[]
}

type NotesState = {
  tree: NoteTreeNode[]
  pages: Record<string, NotePage>
  activePageId?: string
  activeCardId?: string
}

type MaterialPlanEntry = MaterialSummary & {
  sources: ItemSource[]
  choice?: SourceChoice
  source?: ItemSource
  marketable: boolean
  ignored: boolean
  shopName?: string
  gil?: number
  costs?: Array<{ itemId: number; amount: number }>
}

type ExchangePlanGroup = {
  key: string
  shopName: string
  costs: Array<{ itemId: number; amount: number }>
  entries: MaterialPlanEntry[]
}

type DetailTarget = {
  itemId: number
  amountNeeded: number
  recipe?: CraftRecipe
}

type CraftGraphRoot = {
  target: CraftSummaryTarget
  tree: CraftTreeNode
  collapsed: Set<string>
  recipe?: CraftRecipe
}

type MergedCraftGraphNode = {
  itemId: number
  amount: number
  depth: number
  order: number
  recipe?: CraftRecipe
  craftable: boolean
  collapsed: boolean
  root: boolean
}

type MergedCraftGraphEdge = {
  key: string
  from: number
  to: number
  amount: number
  order: number
}

type PositionedCraftGraphNode = MergedCraftGraphNode & {
  x: number
  y: number
}

type PositionedCraftGraphEdge = MergedCraftGraphEdge & {
  fromNode: PositionedCraftGraphNode
  toNode: PositionedCraftGraphNode
  fromOffset: number
  toOffset: number
}

type GraphLayoutRelation = {
  itemId: number
  order: number
}

type CrystalMatrixCell = CrystalResource & {
  amount: number
}

type CrystalMatrixRow = {
  tier: CrystalTier
  total: number
  cells: CrystalMatrixCell[]
}

type SourceDisplayGroup = {
  key: string
  source: ItemSource
  indices: number[]
  costLabel?: string
  details: string[]
}

type MarketQuote = {
  itemId: number
  unitPrice: number
  basis: string
}

type NameDialogState = {
  title: string
  label: string
  initialValue: string
  confirmLabel: string
  onConfirm: (value: string) => void
}

type ConfirmDialogState = {
  title: string
  description: string
  confirmLabel: string
  onConfirm: () => void
}

function id() {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}

function createDefaultPage(pageId: string): NotePage {
  return {
    id: pageId,
    cards: [],
  }
}

function createDefaultState(): NotesState {
  const pageId = id()
  return {
    tree: [{ id: pageId, kind: 'page', title: '新笔记' }],
    pages: { [pageId]: createDefaultPage(pageId) },
    activePageId: pageId,
  }
}

function collectPageIds(node: NoteTreeNode): string[] {
  if (node.kind === 'page') return [node.id]
  return (node.children ?? []).flatMap(collectPageIds)
}

function firstPageId(nodes: NoteTreeNode[]): string | undefined {
  for (const node of nodes) {
    if (node.kind === 'page') return node.id
    const child = firstPageId(node.children ?? [])
    if (child) return child
  }
  return undefined
}

function findTreeNode(nodes: NoteTreeNode[], nodeId: string | undefined): NoteTreeNode | undefined {
  if (!nodeId) return undefined
  for (const node of nodes) {
    if (node.id === nodeId) return node
    const child = findTreeNode(node.children ?? [], nodeId)
    if (child) return child
  }
  return undefined
}

function appendTreeNode(nodes: NoteTreeNode[], parentId: string | undefined, nodeToAdd: NoteTreeNode): NoteTreeNode[] {
  if (!parentId) return [...nodes, nodeToAdd]
  return nodes.map((node) => {
    if (node.kind !== 'folder') return node
    if (node.id === parentId) {
      return { ...node, children: [...(node.children ?? []), nodeToAdd] }
    }
    return { ...node, children: appendTreeNode(node.children ?? [], parentId, nodeToAdd) }
  })
}

function renameTreeNode(nodes: NoteTreeNode[], nodeId: string, title: string): NoteTreeNode[] {
  return nodes.map((node) => {
    if (node.id === nodeId) return { ...node, title }
    if (node.kind === 'folder') return { ...node, children: renameTreeNode(node.children ?? [], nodeId, title) }
    return node
  })
}

function deleteTreeNode(nodes: NoteTreeNode[], nodeId: string): { nodes: NoteTreeNode[]; pageIds: string[] } {
  const pageIds: string[] = []
  const nextNodes: NoteTreeNode[] = []

  for (const node of nodes) {
    if (node.id === nodeId) {
      pageIds.push(...collectPageIds(node))
      continue
    }
    if (node.kind === 'folder') {
      const result = deleteTreeNode(node.children ?? [], nodeId)
      pageIds.push(...result.pageIds)
      nextNodes.push({ ...node, children: result.nodes })
    } else {
      nextNodes.push(node)
    }
  }

  return { nodes: nextNodes, pageIds }
}

function normalizeTarget(raw: any): CraftSummaryTarget | undefined {
  const recipeId = Number(raw?.recipeId)
  const itemId = Number(raw?.itemId)
  const amount = Math.max(1, Math.floor(Number(raw?.amount) || 1))
  if (!Number.isInteger(recipeId) || recipeId <= 0 || !Number.isInteger(itemId) || itemId <= 0) return undefined
  return {
    id: typeof raw?.id === 'string' ? raw.id : id(),
    recipeId,
    itemId,
    amount,
    collapsed: Array.isArray(raw?.collapsed) ? raw.collapsed.filter((value: unknown): value is string => typeof value === 'string') : [],
  }
}

function normalizeCard(raw: any, fallbackTitle: string): CraftSummaryCard | undefined {
  if (raw?.kind === 'craftSummary') {
    const targets = Array.isArray(raw.targets) ? raw.targets.map(normalizeTarget).filter(Boolean) as CraftSummaryTarget[] : []
    return {
      id: typeof raw.id === 'string' ? raw.id : id(),
      kind: 'craftSummary',
      title: typeof raw.title === 'string' && raw.title.trim() ? raw.title : fallbackTitle,
      targets,
      sourceChoices: raw.sourceChoices ?? {},
    }
  }

  const legacyTarget = normalizeTarget(raw)
  if (!legacyTarget) return undefined
  return {
    id: id(),
    kind: 'craftSummary',
    title: fallbackTitle,
    targets: [legacyTarget],
    sourceChoices: {},
  }
}

function normalizePage(raw: any, pageId: string): NotePage {
  if (Array.isArray(raw?.cards)) {
    return {
      id: pageId,
      cards: raw.cards
        .map((card: any, index: number) => normalizeCard(card, `合成汇总 ${index + 1}`))
        .filter(Boolean) as CraftSummaryCard[],
    }
  }

  if (Array.isArray(raw?.sections)) {
    const cards: CraftSummaryCard[] = []
    raw.sections.forEach((section: any, index: number) => {
      const targets = Array.isArray(section?.cards)
        ? section.cards.map(normalizeTarget).filter(Boolean) as CraftSummaryTarget[]
        : []
      if (targets.length === 0) return
      cards.push({
        id: typeof section?.id === 'string' ? section.id : id(),
        kind: 'craftSummary',
        title: typeof section?.title === 'string' && section.title.trim() ? section.title : `合成汇总 ${index + 1}`,
        targets,
        sourceChoices: section?.sourceChoices ?? {},
      })
    })
    return { id: pageId, cards }
  }

  return createDefaultPage(pageId)
}

function normalizeState(raw: Partial<NotesState> | undefined): NotesState {
  const fallback = createDefaultState()
  if (!raw || !Array.isArray(raw.tree) || typeof raw.pages !== 'object' || raw.pages == null) return fallback

  const pageIds = new Set<string>()
  const scan = (nodes: NoteTreeNode[]) => {
    for (const node of nodes) {
      if (!node || typeof node.id !== 'string' || typeof node.title !== 'string') continue
      if (node.kind === 'page') pageIds.add(node.id)
      if (node.kind === 'folder') scan(node.children ?? [])
    }
  }
  scan(raw.tree)

  if (pageIds.size === 0) return fallback

  const pages: Record<string, NotePage> = {}
  for (const pageId of pageIds) {
    pages[pageId] = normalizePage((raw.pages as Record<string, any>)[pageId], pageId)
  }

  const activePageId = raw.activePageId && pageIds.has(raw.activePageId) ? raw.activePageId : firstPageId(raw.tree)
  const activePage = activePageId ? pages[activePageId] : undefined
  const rawActiveCardId = (raw as { activeCardId?: string; activeSectionId?: string }).activeCardId
    ?? (raw as { activeCardId?: string; activeSectionId?: string }).activeSectionId
  const activeCardId = activePage?.cards.some((card) => card.id === rawActiveCardId)
    ? rawActiveCardId
    : activePage?.cards[0]?.id

  return {
    tree: raw.tree,
    pages,
    activePageId,
    activeCardId,
  }
}

function loadNotesState(): NotesState {
  try {
    return normalizeState(JSON.parse(localStorage.getItem(NOTES_STORAGE_KEY) ?? 'null') as Partial<NotesState> | undefined)
  } catch {
    return createDefaultState()
  }
}

function saveNotesState(state: NotesState) {
  localStorage.setItem(NOTES_STORAGE_KEY, JSON.stringify(state))
}

function choiceRecordToMap(record: Record<string, SourceChoice> | undefined): Map<number, SourceChoice> {
  const result = new Map<number, SourceChoice>()
  for (const [itemId, choice] of Object.entries(record ?? {})) {
    const parsed = Number(itemId)
    if (Number.isFinite(parsed) && parsed > 0) result.set(parsed, choice)
  }
  return result
}

function summarizeCardMaterials(engine: CraftDataEngine, card: CraftSummaryCard): MaterialSummary[] {
  const totals = new Map<number, number>()
  for (const target of card.targets) {
    const amount = Math.max(1, Math.floor(target.amount || 1))
    const tree = buildCraftTree(engine, target.itemId, amount)
    const materials = summarizeMaterials(tree, collapsedKeysForTree(tree, new Set(target.collapsed)))
    for (const material of materials) {
      totals.set(material.itemId, (totals.get(material.itemId) ?? 0) + material.amount)
    }
  }
  return [...totals.entries()].map(([itemId, amount]) => ({ itemId, amount }))
}

function graphCollapseKey(itemId: number) {
  return `graph:${itemId}`
}

function collapsedKeysForTree(tree: CraftTreeNode, collapsed: Set<string>) {
  const result = new Set([...collapsed].filter((key) => !key.startsWith('graph:')))
  const visit = (node: CraftTreeNode, depth: number) => {
    if (collapsed.has(graphCollapseKey(node.itemId))) result.add(collapseKey(node.itemId, depth))
    for (const child of node.children) visit(child, depth + 1)
  }
  visit(tree, 0)
  return result
}

function targetWithCollapsedItem(target: CraftSummaryTarget, itemId: number, collapsed: boolean): CraftSummaryTarget {
  const graphKey = graphCollapseKey(itemId)
  const legacyPrefix = `${itemId}:`
  const next = target.collapsed.filter((key) => key !== graphKey && !key.startsWith(legacyPrefix))
  if (collapsed) next.push(graphKey)
  return { ...target, collapsed: next }
}

function parseCrystalResourceName(name: string): CrystalResource | undefined {
  for (const tier of CRYSTAL_TIERS) {
    for (const element of CRYSTAL_ELEMENTS) {
      if (name === `${element}之${tier}`) return { element, tier }
    }
  }
  return undefined
}

function parseCrystalResource(data: CraftDataPackage, itemId: number): CrystalResource | undefined {
  return parseCrystalResourceName(getItemName(data, itemId))
}

function isCrystalResourceLeaf(data: CraftDataPackage, node: CraftTreeNode, parentItemId?: number) {
  return parentItemId != null && node.children.length === 0 && !!parseCrystalResource(data, node.itemId)
}

function createCrystalMatrixRows(totals: Map<string, number>): CrystalMatrixRow[] {
  return CRYSTAL_TIERS.map((tier) => {
    const cells = CRYSTAL_ELEMENTS.map((element) => ({
      element,
      tier,
      amount: totals.get(`${tier}:${element}`) ?? 0,
    }))
    return {
      tier,
      total: cells.reduce((sum, cell) => sum + cell.amount, 0),
      cells,
    }
  })
}

function crystalMatrixFromMaterials(data: CraftDataPackage, materials: MaterialSummary[]): CrystalMatrixRow[] {
  const totals = new Map<string, number>()
  for (const material of materials) {
    const resource = parseCrystalResource(data, material.itemId)
    if (!resource) continue
    const key = `${resource.tier}:${resource.element}`
    totals.set(key, (totals.get(key) ?? 0) + material.amount)
  }
  return createCrystalMatrixRows(totals)
}

function crystalMatrixHasUsage(rows: CrystalMatrixRow[]) {
  return rows.some((row) => row.total > 0)
}

function buildMergedCraftGraph(data: CraftDataPackage, roots: CraftGraphRoot[]) {
  const nodes = new Map<number, MergedCraftGraphNode>()
  const edges = new Map<string, MergedCraftGraphEdge>()
  const rootIds = new Set(roots.map((root) => root.tree.itemId))
  let order = 0

  const visit = (node: CraftTreeNode, depth: number, collapsed: Set<string>, parentItemId?: number, childOrder = 0) => {
    const graphCollapsed = collapsed.has(graphCollapseKey(node.itemId))
    const legacyCollapsed = collapsed.has(collapseKey(node.itemId, depth))
    const isCollapsed = graphCollapsed || legacyCollapsed
    if (isCrystalResourceLeaf(data, node, parentItemId)) return

    const existing = nodes.get(node.itemId)
    if (existing) {
      existing.amount += node.amountNeeded
      existing.depth = Math.max(existing.depth, depth)
      existing.craftable = existing.craftable || node.children.length > 0
      existing.collapsed = existing.collapsed || isCollapsed
      existing.root = existing.root || rootIds.has(node.itemId)
      existing.recipe = existing.recipe ?? node.recipe
    } else {
      const nextNode: MergedCraftGraphNode = {
        itemId: node.itemId,
        amount: node.amountNeeded,
        depth,
        order: order++,
        recipe: node.recipe,
        craftable: node.children.length > 0,
        collapsed: isCollapsed,
        root: rootIds.has(node.itemId),
      }
      nodes.set(node.itemId, nextNode)
    }

    if (parentItemId != null) {
      const edgeKey = `${parentItemId}->${node.itemId}`
      const edge = edges.get(edgeKey)
      if (edge) {
        edge.amount += node.amountNeeded
        edge.order = Math.min(edge.order, childOrder)
      } else {
        edges.set(edgeKey, { key: edgeKey, from: parentItemId, to: node.itemId, amount: node.amountNeeded, order: childOrder })
      }
    }

    if (node.children.length === 0 || isCollapsed) return
    node.children.forEach((child, index) => visit(child, depth + 1, collapsed, node.itemId, index))
  }

  for (const root of roots) visit(root.tree, 0, root.collapsed)

  const depthByItem = new Map<number, number>()
  for (const node of nodes.values()) depthByItem.set(node.itemId, rootIds.has(node.itemId) ? 0 : 0)
  for (let pass = 0; pass < nodes.size; pass += 1) {
    let changed = false
    for (const edge of edges.values()) {
      if (rootIds.has(edge.to)) continue
      const nextDepth = (depthByItem.get(edge.from) ?? 0) + 1
      if (nextDepth > (depthByItem.get(edge.to) ?? 0)) {
        depthByItem.set(edge.to, nextDepth)
        changed = true
      }
    }
    if (!changed) break
  }
  for (const node of nodes.values()) node.depth = depthByItem.get(node.itemId) ?? node.depth

  const sortedNodes = [...nodes.values()].sort((a, b) => a.depth - b.depth || a.order - b.order)
  return {
    nodes: sortedNodes,
    edges: [...edges.values()],
  }
}

function graphLaneIndexes(lanes: Map<number, MergedCraftGraphNode[]>) {
  const indexes = new Map<number, number>()
  for (const lane of lanes.values()) {
    lane.forEach((node, index) => indexes.set(node.itemId, index))
  }
  return indexes
}

function resolvedGraphRelations(relations: GraphLayoutRelation[] | undefined, indexes: Map<number, number>) {
  const resolved: Array<{ index: number; order: number }> = []
  for (const relation of relations ?? []) {
    const index = indexes.get(relation.itemId)
    if (index != null) resolved.push({ index, order: relation.order })
  }
  return resolved
}

function averageResolvedIndex(relations: Array<{ index: number }>) {
  return relations.reduce((sum, relation) => sum + relation.index, 0) / Math.max(1, relations.length)
}

function primaryResolvedOrder(relations: Array<{ index: number; order: number }>) {
  let best: { index: number; order: number } | undefined
  for (const relation of relations) {
    if (!best || relation.index < best.index || (relation.index === best.index && relation.order < best.order)) {
      best = relation
    }
  }
  return best?.order ?? 0
}

function relatedNodeAverage(relations: GraphLayoutRelation[] | undefined, indexes: Map<number, number>) {
  const resolved = resolvedGraphRelations(relations, indexes)
  return resolved.length > 0 ? averageResolvedIndex(resolved) : undefined
}

function relatedNodeSortValue(relations: GraphLayoutRelation[] | undefined, indexes: Map<number, number>, fallbackIndex: number) {
  const resolved = resolvedGraphRelations(relations, indexes)
  if (resolved.length === 0) return fallbackIndex * 1000

  const anchor = averageResolvedIndex(resolved)
  const parentCount = new Set(resolved.map((relation) => relation.index)).size
  const sharedOffset = parentCount > 1 ? 500 : 0
  return anchor * 1000 + sharedOffset + primaryResolvedOrder(resolved)
}

function sortGraphLane(
  lane: MergedCraftGraphNode[],
  related: Map<number, GraphLayoutRelation[]>,
  indexes: Map<number, number>,
) {
  const currentIndexes = new Map(lane.map((node, index) => [node.itemId, index]))
  const hasRelatedScore = lane.some((node) => relatedNodeAverage(related.get(node.itemId), indexes) != null)
  if (!hasRelatedScore) return

  lane.sort((a, b) => {
    const aCurrentIndex = currentIndexes.get(a.itemId) ?? a.order
    const bCurrentIndex = currentIndexes.get(b.itemId) ?? b.order
    return (
      relatedNodeSortValue(related.get(a.itemId), indexes, aCurrentIndex)
      - relatedNodeSortValue(related.get(b.itemId), indexes, bCurrentIndex)
      || aCurrentIndex - bCurrentIndex
      || a.order - b.order
    )
  })
}

function orderedGraphLanes(nodes: MergedCraftGraphNode[], edges: MergedCraftGraphEdge[]) {
  const lanes = new Map<number, MergedCraftGraphNode[]>()
  const nodeIds = new Set(nodes.map((node) => node.itemId))
  const incoming = new Map<number, GraphLayoutRelation[]>()
  const outgoing = new Map<number, GraphLayoutRelation[]>()

  for (const node of nodes) {
    const lane = lanes.get(node.depth) ?? []
    lane.push(node)
    lanes.set(node.depth, lane)
  }
  for (const lane of lanes.values()) lane.sort((a, b) => a.order - b.order)

  for (const edge of edges) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) continue
    const inList = incoming.get(edge.to) ?? []
    inList.push({ itemId: edge.from, order: edge.order })
    incoming.set(edge.to, inList)
    const outList = outgoing.get(edge.from) ?? []
    outList.push({ itemId: edge.to, order: edge.order })
    outgoing.set(edge.from, outList)
  }

  const depths = [...lanes.keys()].sort((a, b) => a - b)
  for (let pass = 0; pass < 6; pass += 1) {
    let indexes = graphLaneIndexes(lanes)
    for (const depth of depths.slice(1)) {
      const lane = lanes.get(depth)
      if (!lane) continue
      sortGraphLane(lane, incoming, indexes)
      indexes = graphLaneIndexes(lanes)
    }

    indexes = graphLaneIndexes(lanes)
    for (const depth of [...depths].reverse().slice(1)) {
      const lane = lanes.get(depth)
      if (!lane) continue
      sortGraphLane(lane, outgoing, indexes)
      indexes = graphLaneIndexes(lanes)
    }
  }

  let indexes = graphLaneIndexes(lanes)
  for (const depth of depths.slice(1)) {
    const lane = lanes.get(depth)
    if (!lane) continue
    sortGraphLane(lane, incoming, indexes)
    indexes = graphLaneIndexes(lanes)
  }

  return lanes
}

function connectedGraphItems(itemId: number, incoming: Map<number, GraphLayoutRelation[]>, outgoing: Map<number, GraphLayoutRelation[]>) {
  return [...(incoming.get(itemId) ?? []), ...(outgoing.get(itemId) ?? [])]
}

function averageConnectedCenter(
  itemId: number,
  incoming: Map<number, GraphLayoutRelation[]>,
  outgoing: Map<number, GraphLayoutRelation[]>,
  centers: Map<number, number>,
) {
  const connected = connectedGraphItems(itemId, incoming, outgoing)
  let total = 0
  let count = 0
  for (const relation of connected) {
    const center = centers.get(relation.itemId)
    if (center == null) continue
    total += center
    count += 1
  }
  return count > 0 ? total / count : undefined
}

function clampGraphPosition(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function laneIdealGap(count: number) {
  if (count >= 11) return 12
  if (count >= 8) return 16
  if (count >= 5) return 22
  if (count >= 3) return 34
  return 48
}

function laneMaxGap(count: number) {
  if (count >= 11) return 34
  if (count >= 8) return 46
  if (count >= 5) return 62
  if (count >= 3) return 82
  return 112
}

function graphContentHeight(lanes: Map<number, MergedCraftGraphNode[]>, nodeHeight: number) {
  let height = 260
  for (const lane of lanes.values()) {
    const count = lane.length
    const laneHeight = count * nodeHeight + Math.max(0, count - 1) * laneIdealGap(count)
    height = Math.max(height, laneHeight)
  }
  return height
}

function initialLaneCenter(
  laneLength: number,
  index: number,
  contentHeight: number,
  nodeHeight: number,
) {
  if (laneLength <= 1) return contentHeight / 2
  const minGap = laneIdealGap(laneLength)
  const availableGap = (contentHeight - laneLength * nodeHeight) / Math.max(1, laneLength - 1)
  const gap = clampGraphPosition(availableGap, minGap, 96)
  const laneHeight = laneLength * nodeHeight + (laneLength - 1) * gap
  const start = (contentHeight - laneHeight) / 2
  return start + nodeHeight / 2 + index * (nodeHeight + gap)
}

function shiftLaneIntoBounds(centers: number[], minCenter: number, maxCenter: number) {
  const overflow = centers[centers.length - 1] - maxCenter
  if (overflow > 0) {
    for (let index = 0; index < centers.length; index += 1) centers[index] -= overflow
  }

  const underflow = minCenter - centers[0]
  if (underflow > 0) {
    for (let index = 0; index < centers.length; index += 1) centers[index] += underflow
  }
}

function enforceLaneMinStep(centers: number[], minStep: number, minCenter: number, maxCenter: number) {
  for (let index = 1; index < centers.length; index += 1) {
    centers[index] = Math.max(centers[index], centers[index - 1] + minStep)
  }

  shiftLaneIntoBounds(centers, minCenter, maxCenter)

  for (let index = centers.length - 2; index >= 0; index -= 1) {
    centers[index] = Math.min(centers[index], centers[index + 1] - minStep)
  }

  shiftLaneIntoBounds(centers, minCenter, maxCenter)
}

function enforceLaneMaxStep(centers: number[], maxStep: number, minCenter: number, maxCenter: number) {
  for (let index = 1; index < centers.length; index += 1) {
    if (centers[index] - centers[index - 1] > maxStep) centers[index] = centers[index - 1] + maxStep
  }

  for (let index = centers.length - 2; index >= 0; index -= 1) {
    if (centers[index + 1] - centers[index] > maxStep) centers[index] = centers[index + 1] - maxStep
  }

  shiftLaneIntoBounds(centers, minCenter, maxCenter)
}

function averageGraphCenters(centers: number[]) {
  return centers.reduce((sum, center) => sum + center, 0) / Math.max(1, centers.length)
}

function resolveLaneCenters(
  lane: MergedCraftGraphNode[],
  centers: Map<number, number>,
  contentHeight: number,
  nodeHeight: number,
) {
  if (lane.length === 0) return
  const minCenter = nodeHeight / 2
  const maxCenter = contentHeight - nodeHeight / 2
  const minStep = nodeHeight + laneIdealGap(lane.length)
  const maxStep = nodeHeight + laneMaxGap(lane.length)
  const desiredCenters = lane.map((node) => clampGraphPosition(centers.get(node.itemId) ?? contentHeight / 2, minCenter, maxCenter))
  const nextCenters = [...desiredCenters]
  const desiredAverage = averageGraphCenters(desiredCenters)

  enforceLaneMinStep(nextCenters, minStep, minCenter, maxCenter)
  for (let pass = 0; pass < 3; pass += 1) {
    enforceLaneMaxStep(nextCenters, maxStep, minCenter, maxCenter)
    enforceLaneMinStep(nextCenters, minStep, minCenter, maxCenter)
  }

  const currentAverage = averageGraphCenters(nextCenters)
  const averageShift = clampGraphPosition(
    desiredAverage - currentAverage,
    minCenter - nextCenters[0],
    maxCenter - nextCenters[nextCenters.length - 1],
  )
  if (averageShift !== 0) {
    for (let index = 0; index < nextCenters.length; index += 1) nextCenters[index] += averageShift
    enforceLaneMinStep(nextCenters, minStep, minCenter, maxCenter)
    enforceLaneMaxStep(nextCenters, maxStep, minCenter, maxCenter)
  }

  lane.forEach((node, index) => centers.set(node.itemId, nextCenters[index]))
}

function adaptiveGraphCenters(
  lanes: Map<number, MergedCraftGraphNode[]>,
  edges: MergedCraftGraphEdge[],
  nodeHeight: number,
) {
  const nodeIds = new Set([...lanes.values()].flat().map((node) => node.itemId))
  const incoming = new Map<number, GraphLayoutRelation[]>()
  const outgoing = new Map<number, GraphLayoutRelation[]>()
  const centers = new Map<number, number>()
  const contentHeight = graphContentHeight(lanes, nodeHeight)
  const depths = [...lanes.keys()].sort((a, b) => a - b)

  for (const edge of edges) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) continue
    incoming.set(edge.to, [...(incoming.get(edge.to) ?? []), { itemId: edge.from, order: edge.order }])
    outgoing.set(edge.from, [...(outgoing.get(edge.from) ?? []), { itemId: edge.to, order: edge.order }])
  }

  for (const depth of depths) {
    const lane = lanes.get(depth) ?? []
    lane.forEach((node, index) => {
      centers.set(node.itemId, initialLaneCenter(lane.length, index, contentHeight, nodeHeight))
    })
  }

  for (let pass = 0; pass < 8; pass += 1) {
    for (const depth of depths) {
      const lane = lanes.get(depth) ?? []
      for (const node of lane) {
        const connectedCenter = averageConnectedCenter(node.itemId, incoming, outgoing, centers)
        if (connectedCenter == null) continue
        const current = centers.get(node.itemId) ?? connectedCenter
        centers.set(node.itemId, current * 0.38 + connectedCenter * 0.62)
      }
      resolveLaneCenters(lane, centers, contentHeight, nodeHeight)
    }

    for (const depth of [...depths].reverse()) {
      const lane = lanes.get(depth) ?? []
      for (const node of lane) {
        const connectedCenter = averageConnectedCenter(node.itemId, incoming, outgoing, centers)
        if (connectedCenter == null) continue
        const current = centers.get(node.itemId) ?? connectedCenter
        centers.set(node.itemId, current * 0.45 + connectedCenter * 0.55)
      }
      resolveLaneCenters(lane, centers, contentHeight, nodeHeight)
    }
  }

  return { centers, contentHeight }
}

function graphEdgePortOffset(index: number, count: number, nodeHeight: number) {
  if (count <= 1) return 0
  const span = Math.min(nodeHeight * 0.62, Math.max(18, (count - 1) * 7))
  return -span / 2 + (span * index) / Math.max(1, count - 1)
}

function applyGraphEdgePorts(edges: PositionedCraftGraphEdge[], nodeHeight: number) {
  const byFrom = new Map<number, PositionedCraftGraphEdge[]>()
  const byTo = new Map<number, PositionedCraftGraphEdge[]>()

  for (const edge of edges) {
    byFrom.set(edge.from, [...(byFrom.get(edge.from) ?? []), edge])
    byTo.set(edge.to, [...(byTo.get(edge.to) ?? []), edge])
  }

  for (const group of byFrom.values()) {
    group
      .sort((a, b) => a.toNode.y - b.toNode.y || a.order - b.order || a.to - b.to)
      .forEach((edge, index) => {
        edge.fromOffset = graphEdgePortOffset(index, group.length, nodeHeight)
      })
  }

  for (const group of byTo.values()) {
    group
      .sort((a, b) => a.fromNode.y - b.fromNode.y || a.order - b.order || a.from - b.from)
      .forEach((edge, index) => {
        edge.toOffset = graphEdgePortOffset(index, group.length, nodeHeight)
      })
  }
}

function recipeLevelLabel(data: CraftDataPackage, recipe: CraftRecipe) {
  if (recipe.secretRecipeBook > 0) {
    return data.secretRecipeBooks[String(recipe.secretRecipeBook)] ?? '秘籍'
  }
  return `Lv.${data.recipeLevels[String(recipe.recipeLevelTableId)]?.classJobLevel ?? 1}`
}

function ItemIcon(props: { icon: number; size?: 'sm' | 'md' }) {
  const urls = () => getIconUrls(props.icon)
  const [index, setIndex] = createSignal(0)
  const sizeClass = () => props.size === 'sm' ? 'h-5 w-5' : 'h-7 w-7'

  return (
    <Show when={urls().length > 0} fallback={<div class={`${sizeClass()} rounded border bg-muted`} />}>
      <img
        src={urls()[index()] ?? urls()[0]}
        alt=""
        loading="lazy"
        class={`${sizeClass()} shrink-0 rounded border bg-muted object-cover`}
        onError={() => {
          if (index() < urls().length - 1) setIndex(index() + 1)
        }}
      />
    </Show>
  )
}
export default function NotesPage() {
  const [craftData] = createResource(loadCraftData)
  const [craftEngine] = createResource(craftData, createCraftDataEngine)
  const [state, setState] = createSignal<NotesState>(loadNotesState())
  const [expandedFolders, setExpandedFolders] = createSignal(new Set<string>())
  const [editingCardId, setEditingCardId] = createSignal<string | 'new' | undefined>()
  const [detailTarget, setDetailTarget] = createSignal<DetailTarget | undefined>()
  const [nameDialog, setNameDialog] = createSignal<NameDialogState | undefined>()
  const [confirmDialog, setConfirmDialog] = createSignal<ConfirmDialogState | undefined>()

  createEffect(() => saveNotesState(state()))

  const recipeById = createMemo(() => {
    const data = craftData()
    return new Map((data?.recipes ?? []).map((recipe) => [recipe.id, recipe]))
  })
  const activePage = createMemo(() => {
    const pageId = state().activePageId
    return pageId ? state().pages[pageId] : undefined
  })
  const activePageNode = createMemo(() => findTreeNode(state().tree, state().activePageId))
  const activeCard = createMemo(() => {
    const page = activePage()
    if (!page) return undefined
    return page.cards.find((card) => card.id === state().activeCardId) ?? page.cards[0]
  })
  const workspaceView = createMemo(() => {
    const data = craftData()
    const engine = craftEngine()
    const page = activePage()
    return data && engine && page ? { data, engine, page } : undefined
  })
  const editorView = createMemo(() => {
    const editorId = editingCardId()
    const data = craftData()
    const engine = craftEngine()
    const page = activePage()
    if (!editorId || !data || !engine || !page) return undefined
    return {
      data,
      engine,
      card: editorId === 'new' ? undefined : page.cards.find((card) => card.id === editorId),
    }
  })
  const detailView = createMemo(() => {
    const target = detailTarget()
    const data = craftData()
    return target && data ? { target, data } : undefined
  })
  const detailRecipe = createMemo(() => {
    const target = detailTarget()
    const engine = craftEngine()
    if (!target) return undefined
    if (target.recipe) return target.recipe
    return engine ? buildCraftTree(engine, target.itemId, target.amountNeeded).recipe : undefined
  })

  const updatePage = (pageId: string, updater: (page: NotePage) => NotePage) => {
    setState((current) => {
      const page = current.pages[pageId]
      if (!page) return current
      return { ...current, pages: { ...current.pages, [pageId]: updater(page) } }
    })
  }
  const updateActivePage = (updater: (page: NotePage) => NotePage) => {
    const page = activePage()
    if (page) updatePage(page.id, updater)
  }
  const createFolder = (title: string, parentId?: string) => {
    const folderId = id()
    setState((current) => ({
      ...current,
      tree: appendTreeNode(current.tree, parentId, { id: folderId, kind: 'folder', title, children: [] }),
    }))
    setExpandedFolders((current) => new Set(parentId ? [...current, parentId, folderId] : [...current, folderId]))
  }
  const createPage = (title: string, parentId?: string) => {
    const pageId = id()
    const page = createDefaultPage(pageId)
    setState((current) => ({
      ...current,
      tree: appendTreeNode(current.tree, parentId, { id: pageId, kind: 'page', title }),
      pages: { ...current.pages, [pageId]: page },
      activePageId: pageId,
      activeCardId: undefined,
    }))
    if (parentId) setExpandedFolders((current) => new Set([...current, parentId]))
  }
  const deleteNode = (nodeId: string) => {
    setState((current) => {
      const result = deleteTreeNode(current.tree, nodeId)
      const pages = { ...current.pages }
      for (const pageId of result.pageIds) delete pages[pageId]
      const activePageId = current.activePageId && !result.pageIds.includes(current.activePageId)
        ? current.activePageId
        : firstPageId(result.nodes)
      return {
        tree: result.nodes,
        pages,
        activePageId,
        activeCardId: activePageId ? pages[activePageId]?.cards[0]?.id : undefined,
      }
    })
  }
  const addFolder = (parentId?: string) => {
    setNameDialog({
      title: '新建目录',
      label: '目录名称',
      initialValue: '新目录',
      confirmLabel: '创建',
      onConfirm: (title) => createFolder(title, parentId),
    })
  }
  const addPage = (parentId?: string) => {
    setNameDialog({
      title: '新建页面',
      label: '页面名称',
      initialValue: '新页面',
      confirmLabel: '创建',
      onConfirm: (title) => createPage(title, parentId),
    })
  }
  const renameNode = (nodeId: string, currentTitle: string) => {
    setNameDialog({
      title: '重命名',
      label: '名称',
      initialValue: currentTitle,
      confirmLabel: '保存',
      onConfirm: (title) => setState((current) => ({ ...current, tree: renameTreeNode(current.tree, nodeId, title) })),
    })
  }
  const removeNode = (nodeId: string) => {
    const node = findTreeNode(state().tree, nodeId)
    if (!node) return
    setConfirmDialog({
      title: `删除${node.kind === 'folder' ? '目录' : '页面'}`,
      description: node.kind === 'folder' ? `将删除“${node.title}”以及其中所有页面。` : `将删除页面“${node.title}”。`,
      confirmLabel: '删除',
      onConfirm: () => deleteNode(nodeId),
    })
  }
  const selectPage = (pageId: string) => {
    const page = state().pages[pageId]
    setState((current) => ({ ...current, activePageId: pageId, activeCardId: page?.cards[0]?.id }))
  }
  const saveCard = (card: CraftSummaryCard) => {
    const page = activePage()
    if (!page) return
    updatePage(page.id, (current) => {
      const exists = current.cards.some((item) => item.id === card.id)
      return {
        ...current,
        cards: exists ? current.cards.map((item) => item.id === card.id ? card : item) : [...current.cards, card],
      }
    })
    setState((current) => ({ ...current, activeCardId: card.id }))
  }
  const removeCard = (cardId: string) => {
    const page = activePage()
    const card = page?.cards.find((item) => item.id === cardId)
    if (!page || !card) return
    setConfirmDialog({
      title: '删除合成汇总卡片',
      description: `将删除“${card.title}”。`,
      confirmLabel: '删除',
      onConfirm: () => {
        setState((current) => {
          const currentPage = current.pages[page.id]
          if (!currentPage) return current
          const cards = currentPage.cards.filter((item) => item.id !== cardId)
          return {
            ...current,
            activeCardId: current.activeCardId === cardId ? cards[0]?.id : current.activeCardId,
            pages: { ...current.pages, [page.id]: { ...currentPage, cards } },
          }
        })
      },
    })
  }
  const updateCard = (cardId: string, updater: (card: CraftSummaryCard) => CraftSummaryCard) => {
    updateActivePage((page) => ({
      ...page,
      cards: page.cards.map((card) => card.id === cardId ? updater(card) : card),
    }))
  }
  const chooseCardSource = (cardId: string, itemId: number, choice: SourceChoice | undefined) => {
    updateCard(cardId, (card) => {
      const sourceChoices = { ...(card.sourceChoices ?? {}) }
      if (choice) sourceChoices[String(itemId)] = choice
      else delete sourceChoices[String(itemId)]
      return { ...card, sourceChoices }
    })
  }
  const toggleFolder = (folderId: string) => {
    setExpandedFolders((current) => {
      const next = new Set(current)
      if (next.has(folderId)) next.delete(folderId)
      else next.add(folderId)
      return next
    })
  }

  return (
    <div class="flex min-h-screen flex-col lg:h-screen lg:min-h-0 lg:overflow-hidden">
      <div class="shrink-0 border-b bg-background px-4 py-4 sm:px-6 lg:px-8">
        <div class="mx-auto flex max-w-[1720px] flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <div class="text-sm text-muted-foreground">工具 / 笔记</div>
            <h1 class="text-2xl font-semibold">笔记</h1>
          </div>
          <Show when={craftData()}>
            {(data) => (
              <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
                <Badge variant="outline">配方 {formatInteger(data().counts.recipes)}</Badge>
                <Badge variant="outline">物品 {formatInteger(data().counts.items)}</Badge>
                <Badge variant="outline">来源 {formatInteger(data().counts.sources)}</Badge>
              </div>
            )}
          </Show>
        </div>
      </div>

      <div class="grid w-full flex-1 lg:min-h-0 xl:grid-cols-[280px_minmax(0,1fr)] 2xl:grid-cols-[300px_minmax(0,1fr)]">
        <aside class="flex h-[320px] flex-col overflow-hidden border-b bg-card xl:h-auto xl:min-h-0 xl:border-b-0 xl:border-r">
          <div class="flex items-center justify-between gap-2 border-b p-3">
            <div class="flex items-center gap-2 text-sm font-semibold">
              <Folder class="h-4 w-4" />
              笔记树
            </div>
            <div class="flex gap-1">
              <Button size="icon" variant="ghost" title="添加页面" aria-label="添加页面" onClick={() => addPage()}>
                <FilePlus2 class="h-4 w-4" />
              </Button>
              <Button size="icon" variant="ghost" title="添加目录" aria-label="添加目录" onClick={() => addFolder()}>
                <FolderPlus class="h-4 w-4" />
              </Button>
            </div>
          </div>
          <div class="min-h-0 flex-1 overflow-y-auto p-2">
            <For each={state().tree}>
              {(node) => (
                <TreeNodeRow
                  node={node}
                  depth={0}
                  activePageId={state().activePageId}
                  expanded={expandedFolders()}
                  onToggle={toggleFolder}
                  onSelectPage={selectPage}
                  onAddFolder={addFolder}
                  onAddPage={addPage}
                  onRename={renameNode}
                  onDelete={removeNode}
                />
              )}
            </For>
          </div>
        </aside>

        <main class="min-h-[560px] overflow-hidden bg-background lg:min-h-0">
          <Show
            when={workspaceView()}
            fallback={<div class="p-4"><EmptyState icon={<BookOpen class="h-6 w-6" />} title="笔记未载入" /></div>}
          >
            {(view) => (
              <div class="flex h-full min-h-[560px] flex-col lg:min-h-0">
                <div class="flex flex-wrap items-center justify-between gap-3 border-b p-4">
                  <div class="min-w-0">
                    <div class="truncate text-base font-semibold">{activePageNode()?.title ?? '未命名页面'}</div>
                    <div class="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                      <Badge variant="outline"><Hammer class="mr-1 h-3 w-3" />汇总卡片 {view().page.cards.length}</Badge>
                    </div>
                  </div>
                  <Button variant="primary" onClick={() => setEditingCardId('new')}>
                    <Plus class="h-4 w-4" />
                    添加汇总卡片
                  </Button>
                </div>

                <div class="min-h-0 flex-1 overflow-y-auto p-3">
                  <Show
                    when={view().page.cards.length > 0}
                    fallback={
                      <EmptyState
                        icon={<PackageSearch class="h-6 w-6" />}
                        title="还没有合成汇总卡片"
                        description="创建一张卡片后，可以在里面搜索并多选要制作的物品。"
                        action={<Button variant="primary" onClick={() => setEditingCardId('new')}><Plus class="h-4 w-4" />添加汇总卡片</Button>}
                      />
                    }
                  >
                    <div class="grid w-full gap-3">
                      <For each={view().page.cards}>
                        {(card) => (
                          <CraftSummaryCardView
                            data={view().data}
                            engine={view().engine}
                            card={card}
                            recipes={recipeById()}
                            active={activeCard()?.id === card.id}
                            sourceChoices={choiceRecordToMap(card.sourceChoices)}
                            onSelect={() => setState((current) => ({ ...current, activeCardId: card.id }))}
                            onEdit={() => setEditingCardId(card.id)}
                            onRemove={() => removeCard(card.id)}
                            onToggleCollapsedItem={(itemId, collapsed) => updateCard(card.id, (current) => ({
                              ...current,
                              targets: current.targets.map((target) => targetWithCollapsedItem(target, itemId, collapsed)),
                            }))}
                            onChooseSource={(itemId, choice) => chooseCardSource(card.id, itemId, choice)}
                            onInspect={setDetailTarget}
                          />
                        )}
                      </For>
                    </div>
                  </Show>
                </div>
              </div>
            )}
          </Show>
        </main>
      </div>

      <Show when={editorView()}>
        {(view) => (
          <CraftSummaryEditorDialog
            data={view().data}
            engine={view().engine}
            card={view().card}
            onSave={saveCard}
            onClose={() => setEditingCardId(undefined)}
          />
        )}
      </Show>

      <Show when={detailView()}>
        {(view) => (
          <ItemDetailDialog
            data={view().data}
            target={view().target}
            recipe={detailRecipe()}
            onClose={() => setDetailTarget(undefined)}
          />
        )}
      </Show>

      <Show when={nameDialog()}>
        {(dialog) => <NameDialog {...dialog()} onClose={() => setNameDialog(undefined)} />}
      </Show>

      <Show when={confirmDialog()}>
        {(dialog) => <ConfirmDialog {...dialog()} onClose={() => setConfirmDialog(undefined)} />}
      </Show>
    </div>
  )
}

function sourceIcon(source: ItemSource) {
  if (source.kind === 'gilShop') return <Coins class="h-3.5 w-3.5 shrink-0" />
  if (source.kind === 'specialShop') return <Shuffle class="h-3.5 w-3.5 shrink-0" />
  if (source.kind === 'fishing') return <Fish class="h-3.5 w-3.5 shrink-0" />
  return <Leaf class="h-3.5 w-3.5 shrink-0" />
}

function sourceGilCost(data: CraftDataPackage, itemId: number, amount: number) {
  const unitPrice = getItem(data, itemId)?.priceMid ?? 0
  return unitPrice > 0 ? unitPrice * amount : undefined
}

function costListLabel(data: CraftDataPackage, costs: Array<{ itemId: number; amount: number }>) {
  return costs
    .map((cost) => `${getItemName(data, cost.itemId)} x${formatInteger(cost.amount)}`)
    .join(' + ')
}

function sourceCostLabel(data: CraftDataPackage, source: ItemSource, amount: number, itemId?: number) {
  if (source.kind === 'gilShop' && itemId) {
    const gil = sourceGilCost(data, itemId, amount)
    return gil ? `${formatInteger(gil)}G` : undefined
  }
  if (source.kind !== 'specialShop') return undefined
  return costListLabel(
    data,
    source.costs.map((cost) => ({ itemId: cost.itemId, amount: cost.count * amount })),
  )
}

function sourceInfoLabel(source: ItemSource) {
  if (source.kind === 'gilShop' || source.kind === 'specialShop') return source.shopName
  if (source.kind === 'fishing') return `钓场 #${source.spotId}`
  return '采矿 / 园艺'
}

function sourceConsumptionKey(data: CraftDataPackage, source: ItemSource, amount: number, itemId: number) {
  if (source.kind === 'gilShop') return `gilShop|${sourceGilCost(data, itemId, amount) ?? 0}`
  if (source.kind === 'specialShop') {
    return `specialShop|${source.costs
      .map((cost) => `${cost.itemId}:${cost.count * amount}`)
      .sort()
      .join(',')}`
  }
  if (source.kind === 'fishing') return `fishing|${source.fishId}|${source.spotId}`
  return 'gathering'
}

function sourceDisplayGroups(
  data: CraftDataPackage,
  itemId: number,
  sources: ItemSource[],
  amount: number,
): SourceDisplayGroup[] {
  const groups = new Map<string, SourceDisplayGroup>()

  sources.forEach((source, index) => {
    const key = sourceConsumptionKey(data, source, amount, itemId)
    let group = groups.get(key)
    if (!group) {
      group = {
        key,
        source,
        indices: [],
        costLabel: sourceCostLabel(data, source, amount, itemId),
        details: [],
      }
      groups.set(key, group)
    }
    group.indices.push(index)
    group.details.push(sourceInfoLabel(source))
  })

  return [...groups.values()].sort((a, b) => sourcePriority(a.source) - sourcePriority(b.source))
}

function sourceToneClass(source: ItemSource | undefined, ignored = false) {
  if (ignored) return 'border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground'
  if (source?.kind === 'gathering') return 'border-l-emerald-200 bg-emerald-50/80'
  if (source?.kind === 'fishing') return 'border-l-cyan-200 bg-cyan-50/80'
  if (source?.kind === 'specialShop') return 'border-l-[#d7c7ff] bg-[#f5efff]'
  if (source?.kind === 'gilShop') return 'border-l-amber-200 bg-amber-50/80'
  return 'border-l-border bg-background'
}

function sourceButtonClass(source: ItemSource | undefined, active: boolean) {
  if (!active) return 'border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground'
  if (source?.kind === 'gathering') return 'border-emerald-200 bg-[#dff5e5] text-[#166534]'
  if (source?.kind === 'fishing') return 'border-cyan-200 bg-[#d8f3fb] text-[#155e75]'
  if (source?.kind === 'specialShop') return 'border-[#bfa7ff] bg-[#e4d9ff] text-[#3b2778]'
  if (source?.kind === 'gilShop') return 'border-amber-200 bg-[#fff0bf] text-[#854d0e]'
  return 'border-border bg-secondary text-secondary-foreground'
}

function marketButtonClass(active: boolean) {
  if (active) return 'border-[#93c5fd] bg-[#dbeafe] text-[#1d4ed8]'
  return 'border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground'
}

function isMarketable(item?: CraftItem) {
  return (item?.itemSearchCategory ?? 0) > 0
}

function exchangeGroupKey(source: Extract<ItemSource, { kind: 'specialShop' }>) {
  return `${source.shopName}|${source.costs.map((cost) => cost.itemId).join('+')}`
}

function firstPositive(...values: Array<number | undefined>) {
  return values.find((value) => typeof value === 'number' && Number.isFinite(value) && value > 0)
}

function normalizeMarketQuote(raw: any): MarketQuote | undefined {
  const minListingPrice = firstPositive(raw.nq?.minListing?.region?.price)
  const recentPurchasePrice = firstPositive(raw.nq?.recentPurchase?.region?.price)
  const averageSalePrice = firstPositive(raw.nq?.averageSalePrice?.region?.price)
  const unitPrice = firstPositive(minListingPrice, recentPurchasePrice, averageSalePrice)
  if (!unitPrice) return undefined

  return {
    itemId: Number(raw.itemId),
    unitPrice,
    basis: minListingPrice ? '最低挂单' : recentPurchasePrice ? '近期成交' : '平均成交',
  }
}

async function fetchMarketQuotes(itemIdsKey: string): Promise<Map<number, MarketQuote>> {
  const itemIds = itemIdsKey.split(',').map(Number).filter((itemId) => Number.isFinite(itemId) && itemId > 0)
  const result = new Map<number, MarketQuote>()
  const chunks: number[][] = []
  for (let i = 0; i < itemIds.length; i += 100) chunks.push(itemIds.slice(i, i + 100))

  await Promise.all(chunks.map(async (chunk) => {
    const res = await fetch(
      `${UNIVERSALIS_BASE_URL}/api/v2/aggregated/${encodeURIComponent(MARKET_WORLD_DC_REGION)}/${chunk.join(',')}`,
    )
    if (!res.ok) throw new Error(`Universalis ${res.status}`)
    const json = await res.json()
    for (const raw of json.results ?? []) {
      const quote = normalizeMarketQuote(raw)
      if (quote) result.set(quote.itemId, quote)
    }
  }))

  return result
}

function leafToneClass(data: CraftDataPackage, node: CraftTreeNode, choices: Map<number, SourceChoice>) {
  const sources = data.sources[String(node.itemId)] ?? []
  const choice = choices.get(node.itemId)
  const item = getItem(data, node.itemId)
  const marketable = isMarketable(item)
  const source = resolveSource(node.itemId, sources, choices)

  if (choice?.kind === 'ignore') return 'border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground hover:bg-[#e7e5e4]'
  if (marketable && choice?.kind === 'market') return 'border-l-[#93c5fd] bg-[#eff6ff] hover:bg-[#dbeafe]'
  if (source?.kind === 'gilShop') return 'border-l-amber-200 bg-amber-50/80 hover:bg-amber-100/70'
  if (source?.kind === 'specialShop') return 'border-l-[#d7c7ff] bg-[#f7f2ff] hover:bg-[#efe7ff]'
  if (source?.kind === 'gathering' || source?.kind === 'fishing') return 'border-l-emerald-200 bg-emerald-50/80 hover:bg-emerald-100/70'
  return 'border-l-border bg-background hover:bg-muted/35'
}

function huijiItemUrl(itemName: string) {
  return `https://ff14.huijiwiki.com/wiki/${encodeURIComponent(`物品:${itemName}`)}`
}

function lodestoneItemSearchUrl(itemName: string) {
  return `https://na.finalfantasyxiv.com/lodestone/playguide/db/search/?q=${encodeURIComponent(itemName)}`
}

function xivMarketItemUrl(itemId: number) {
  return `https://azurice.github.io/xiv-market/#/item/${itemId}`
}

function fishCakeUrl(spotId: number, fishId: number) {
  return `https://fish.ffmomola.com/#/wiki/fishing/spot/${spotId}/fish/${fishId}`
}

function ExternalItemLink(props: { href: string; label: string }) {
  return (
    <a
      href={props.href}
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex h-7 items-center gap-1 rounded border bg-background px-2 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
    >
      <ExternalLink class="h-3 w-3 shrink-0" />
      {props.label}
    </a>
  )
}

function ItemExternalLinks(props: {
  item?: CraftItem
  itemId: number
  itemName: string
  sources: ItemSource[]
}) {
  const fishing = () => props.sources.find((source): source is Extract<ItemSource, { kind: 'fishing' }> => source.kind === 'fishing')

  return (
    <div class="flex flex-wrap gap-1.5">
      <ExternalItemLink href={huijiItemUrl(props.itemName)} label="灰机 Wiki" />
      <ExternalItemLink href={lodestoneItemSearchUrl(props.itemName)} label="Lodestone" />
      <Show when={(props.item?.itemSearchCategory ?? 0) > 0}>
        <ExternalItemLink href={xivMarketItemUrl(props.itemId)} label="xiv-market" />
      </Show>
      <Show when={fishing()}>
        {(source) => <ExternalItemLink href={fishCakeUrl(source().spotId, source().fishId)} label="鱼糕" />}
      </Show>
    </div>
  )
}

function SourceChoiceControls(props: {
  data: CraftDataPackage
  itemId: number
  amount: number
  sources: ItemSource[]
  marketable: boolean
  choice?: SourceChoice
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
}) {
  const ignored = () => props.choice?.kind === 'ignore'
  const market = () => props.choice?.kind === 'market'
  const currentSourceIndex = () => {
    if (props.choice?.kind === 'index') return props.choice.index
    if (props.choice?.kind === 'ignore' || props.choice?.kind === 'market') return undefined
    return defaultSourceIndex(props.sources)
  }
  const groups = () => sourceDisplayGroups(props.data, props.itemId, props.sources, props.amount)

  return (
    <div class="flex flex-wrap gap-1.5" onClick={(event) => event.stopPropagation()}>
      <Show when={props.marketable}>
        <button
          type="button"
          class={cx(
            'inline-flex min-h-7 max-w-full items-center gap-1 rounded border px-2 py-1 text-left text-[11px] font-medium leading-snug transition-colors',
            marketButtonClass(market()),
          )}
          onClick={() => props.onChoose(props.itemId, market() ? undefined : { kind: 'market' })}
          title={`从市场购买（${MARKET_WORLD_DC_REGION}）`}
          aria-label="市场购买"
        >
          <Coins class="h-3 w-3 shrink-0" />
          市场
        </button>
      </Show>

      <For each={groups()}>
        {(group) => {
          const active = () => {
            const sourceIndex = currentSourceIndex()
            return !ignored() && !market() && sourceIndex != null && group.indices.includes(sourceIndex)
          }
          return (
            <button
              type="button"
              class={cx(
                'inline-flex min-h-7 max-w-full items-center gap-1 rounded border px-2 py-1 text-left text-[11px] font-medium leading-snug transition-colors',
                sourceButtonClass(group.source, active()),
              )}
              onClick={() => props.onChoose(props.itemId, { kind: 'index', index: group.indices[0] ?? 0 })}
              title={group.details.join('\n')}
            >
              {sourceIcon(group.source)}
              <span>{sourceLabel(group.source)}</span>
              <Show when={group.costLabel}>
                {(label) => (
                  <span class={cx('min-w-0 whitespace-normal break-words', active() ? 'opacity-90' : 'opacity-70')}>
                    {label()}
                  </span>
                )}
              </Show>
            </button>
          )
        }}
      </For>

      <Show when={!props.marketable && props.sources.length === 0}>
        <span class="inline-flex min-h-7 items-center rounded border bg-background/80 px-2 py-1 text-[11px] text-muted-foreground">无来源</span>
      </Show>
    </div>
  )
}

function MaterialPlanRow(props: {
  data: CraftDataPackage
  entry: MaterialPlanEntry
  class: string
  meta?: JSX.Element
  subdued?: boolean
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
  onInspect?: (entry: MaterialPlanEntry) => void
}) {
  const item = () => getItem(props.data, props.entry.itemId)
  const owned = () => props.entry.choice?.kind === 'ignore'

  return (
    <div
      class={cx(
        'rounded-sm border-l-2 px-2 py-2 text-sm',
        props.onInspect && 'cursor-pointer transition-[box-shadow,filter] hover:shadow-sm hover:brightness-[0.98]',
        props.class,
        props.subdued && 'opacity-75',
      )}
      onClick={() => props.onInspect?.(props.entry)}
    >
      <div class="grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm">
        <ItemIcon icon={item()?.icon ?? 0} size="sm" />
        <div class="min-w-0">
          <div class="truncate font-medium">{getItemName(props.data, props.entry.itemId)}</div>
          <Show when={props.meta}>
            <div class="whitespace-normal break-words text-xs leading-snug text-muted-foreground">{props.meta}</div>
          </Show>
        </div>
        <div class="flex items-center gap-1.5">
          <Badge variant="outline" class="bg-background/70">x{formatInteger(props.entry.amount)}</Badge>
          <button
            type="button"
            class={cx(
              'flex h-7 w-7 items-center justify-center rounded-full border transition-colors',
              owned()
                ? 'border-[#a8a29e] bg-[#e7e5e4] text-[#44403c]'
                : 'border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground',
            )}
            title={owned() ? '取消已拥有' : '标记为已拥有'}
            aria-label={owned() ? '取消已拥有' : '标记为已拥有'}
            onClick={(event) => {
              event.stopPropagation()
              props.onChoose(props.entry.itemId, owned() ? undefined : { kind: 'ignore' })
            }}
          >
            <CircleCheck class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <div class="mt-2 pl-7">
        <SourceChoiceControls
          data={props.data}
          itemId={props.entry.itemId}
          amount={props.entry.amount}
          sources={props.entry.sources}
          marketable={props.entry.marketable}
          choice={props.entry.choice}
          onChoose={props.onChoose}
        />
      </div>
    </div>
  )
}

function SummaryItemRow(props: {
  data: CraftDataPackage
  itemId: number
  amount: number
  class: string
  onInspect?: (itemId: number, amount: number) => void
}) {
  const item = () => getItem(props.data, props.itemId)

  return (
    <div
      class={cx(
        'mb-1 grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm border-l-2 px-2 py-1.5 text-sm',
        props.onInspect && 'cursor-pointer transition-shadow hover:shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        props.class,
      )}
      role={props.onInspect ? 'button' : undefined}
      tabIndex={props.onInspect ? 0 : undefined}
      onClick={() => props.onInspect?.(props.itemId, props.amount)}
      onKeyDown={(event) => {
        if (!props.onInspect || (event.key !== 'Enter' && event.key !== ' ')) return
        event.preventDefault()
        props.onInspect(props.itemId, props.amount)
      }}
    >
      <ItemIcon icon={item()?.icon ?? 0} size="sm" />
      <div class="min-w-0">
        <div class="truncate font-medium">{getItemName(props.data, props.itemId)}</div>
      </div>
      <Badge variant="outline" class="bg-background/70">x{formatInteger(props.amount)}</Badge>
    </div>
  )
}

function ExchangeGroupPanel(props: {
  data: CraftDataPackage
  group: ExchangePlanGroup
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
  onInspect: (entry: MaterialPlanEntry) => void
  onInspectItem: (itemId: number, amount: number) => void
}) {
  return (
    <div class="overflow-hidden rounded-md border bg-[#fbf9ff]">
      <div class="grid grid-cols-1 2xl:grid-cols-[160px_minmax(0,1fr)]">
        <div class="border-b bg-[#f0ebff] p-3 2xl:border-b-0 2xl:border-r">
          <div class="mb-2 min-w-0 break-words text-xs font-medium text-[#4c3290]">{props.group.shopName}</div>
          <div class="space-y-1.5">
            <For each={props.group.costs}>
              {(cost) => (
                <SummaryItemRow
                  data={props.data}
                  itemId={cost.itemId}
                  amount={cost.amount}
                  class="border-l-[#bfa7ff] bg-background/75"
                  onInspect={props.onInspectItem}
                />
              )}
            </For>
          </div>
        </div>

        <div class="divide-y">
          <For each={props.group.entries}>
            {(entry) => (
              <div class="p-2">
                <MaterialPlanRow
                  data={props.data}
                  entry={entry}
                  class="border-l-[#d7c7ff] bg-[#f7f2ff]"
                  meta={entry.costs ? costListLabel(props.data, entry.costs) : undefined}
                  subdued
                  onChoose={props.onChoose}
                  onInspect={props.onInspect}
                />
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  )
}

function MaterialSummaryPanel(props: {
  data: CraftDataPackage
  title?: string
  materials: MaterialSummary[]
  sourceChoices: Map<number, SourceChoice>
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
  onInspectItem: (itemId: number, amount: number) => void
}) {
  const crystalRows = createMemo(() => crystalMatrixFromMaterials(props.data, props.materials))
  const crystalTotal = () => crystalRows().reduce((sum, row) => sum + row.total, 0)
  const crystalUsage = () => crystalMatrixHasUsage(crystalRows())
  const ordinaryMaterialCount = () => props.materials.filter((material) => !parseCrystalResource(props.data, material.itemId)).length
  const materialPlan = createMemo(() => {
    const gathering: MaterialPlanEntry[] = []
    const shops: MaterialPlanEntry[] = []
    const market: MaterialPlanEntry[] = []
    const owned: MaterialPlanEntry[] = []
    const unknown: MaterialPlanEntry[] = []
    const exchangeGroups = new Map<string, ExchangePlanGroup & { costMap: Map<number, number> }>()
    let gilTotal = 0

    for (const material of props.materials) {
      if (parseCrystalResource(props.data, material.itemId)) continue

      const sources = props.data.sources[String(material.itemId)] ?? []
      const item = getItem(props.data, material.itemId)
      const marketable = isMarketable(item)
      const choice = props.sourceChoices.get(material.itemId)
      const source = resolveSource(material.itemId, sources, props.sourceChoices)
      const baseEntry: MaterialPlanEntry = {
        ...material,
        sources,
        choice,
        source,
        marketable,
        ignored: choice?.kind === 'ignore',
      }

      if (choice?.kind === 'ignore') {
        owned.push(baseEntry)
        continue
      }
      if (marketable && choice?.kind === 'market') {
        market.push({ ...baseEntry, source: undefined })
        continue
      }
      if (source?.kind === 'gilShop') {
        const gil = (getItem(props.data, material.itemId)?.priceMid ?? 0) * material.amount
        gilTotal += gil
        shops.push({ ...baseEntry, shopName: source.shopName, gil })
      } else if (source?.kind === 'specialShop') {
        const costs = source.costs.map((cost) => ({
          itemId: cost.itemId,
          amount: cost.count * material.amount,
        }))
        const key = exchangeGroupKey(source)
        let group = exchangeGroups.get(key)
        if (!group) {
          group = {
            key,
            shopName: source.shopName,
            costs: [],
            entries: [],
            costMap: new Map<number, number>(),
          }
          exchangeGroups.set(key, group)
        }
        group.entries.push({ ...baseEntry, shopName: source.shopName, costs })
        for (const cost of costs) {
          group.costMap.set(cost.itemId, (group.costMap.get(cost.itemId) ?? 0) + cost.amount)
        }
      } else if (source?.kind === 'gathering' || source?.kind === 'fishing') {
        gathering.push(baseEntry)
      } else {
        unknown.push(baseEntry)
      }
    }

    return {
      gathering,
      shops,
      market,
      exchangeGroups: [...exchangeGroups.values()].map((group) => ({
        key: group.key,
        shopName: group.shopName,
        entries: group.entries,
        costs: [...group.costMap.entries()]
          .map(([itemId, amount]) => ({ itemId, amount }))
          .sort((a, b) => a.itemId - b.itemId),
      })),
      owned,
      unknown,
      gilTotal,
    }
  })

  const marketItemIdsKey = createMemo(() => {
    const ids = [...new Set(materialPlan().market.map((entry) => entry.itemId))].sort((a, b) => a - b)
    return ids.length > 0 ? ids.join(',') : undefined
  })
  const [marketQuotes] = createResource(marketItemIdsKey, fetchMarketQuotes)
  const marketCost = createMemo(() => {
    const quotes = marketQuotes()
    let total = 0
    for (const entry of materialPlan().market) {
      const unitPrice = quotes?.get(entry.itemId)?.unitPrice
      if (unitPrice) total += unitPrice * entry.amount
    }
    return total
  })
  const inspectEntry = (entry: MaterialPlanEntry) => props.onInspectItem(entry.itemId, entry.amount)
  const marketMeta = (entry: MaterialPlanEntry) => {
    if (marketQuotes.loading) return `估价载入中 · ${MARKET_WORLD_DC_REGION}`
    if (marketQuotes.error) return '估价失败'
    const quote = marketQuotes()?.get(entry.itemId)
    if (!quote) return '暂无市场价格'
    return `${formatInteger(quote.unitPrice)}G / 个 · ${formatInteger(quote.unitPrice * entry.amount)}G · ${quote.basis}`
  }
  const empty = () => (
    materialPlan().exchangeGroups.length === 0
    && materialPlan().gathering.length === 0
    && materialPlan().shops.length === 0
    && materialPlan().market.length === 0
    && materialPlan().unknown.length === 0
    && materialPlan().owned.length === 0
    && !crystalUsage()
  )

  return (
    <section class="overflow-hidden rounded-md border bg-background">
      <div class="shrink-0 border-b p-3">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="flex items-center gap-2 text-sm font-semibold">
              <Info class="h-4 w-4" />
              材料清单
            </div>
            <div class="mt-1 truncate text-xs text-muted-foreground">{props.title ?? '当前汇总卡片'}</div>
          </div>
          <div class="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
            <Badge variant="outline">材料 {ordinaryMaterialCount()}</Badge>
            <Show when={crystalUsage()}>
              <Badge variant="outline">晶石 {formatInteger(crystalTotal())}</Badge>
            </Show>
          </div>
        </div>

        <div class="mt-3 flex flex-wrap gap-2">
          <Show when={crystalUsage()}>
            <Badge variant="outline">晶石汇总 {formatInteger(crystalTotal())}</Badge>
          </Show>
          <Show when={materialPlan().gathering.length > 0}>
            <Badge variant="success"><Leaf class="mr-1 h-3 w-3" />采集 {materialPlan().gathering.length}</Badge>
          </Show>
          <Show when={materialPlan().exchangeGroups.length > 0}>
            <Badge variant="outline"><Shuffle class="mr-1 h-3 w-3" />兑换 {materialPlan().exchangeGroups.length}</Badge>
          </Show>
          <Show when={materialPlan().gilTotal > 0}>
            <Badge variant="warning"><Coins class="mr-1 h-3 w-3" />商店 {formatInteger(materialPlan().gilTotal)}G</Badge>
          </Show>
          <Show when={materialPlan().market.length > 0}>
            <Badge variant="outline"><Coins class="mr-1 h-3 w-3" />市场 {materialPlan().market.length}</Badge>
          </Show>
          <Show when={marketCost() > 0}>
            <Badge variant="outline">估 {formatInteger(marketCost())}G</Badge>
          </Show>
          <Show when={materialPlan().owned.length > 0}>
            <Badge variant="outline"><CircleCheck class="mr-1 h-3 w-3" />已拥有 {materialPlan().owned.length}</Badge>
          </Show>
        </div>
      </div>

      <div class="p-3">
        <Show
          when={!empty()}
          fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="暂无材料" description="卡片里选择物品后会汇总叶子材料" />}
        >
          <div class="space-y-4">
            <Show when={crystalUsage()}>
              <section>
                <CrystalCostMatrix rows={crystalRows()} />
              </section>
            </Show>

            <Show when={materialPlan().exchangeGroups.length > 0}>
              <section>
                <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                  <Shuffle class="h-4 w-4 text-[#3b2778]" />
                  兑换
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().exchangeGroups}>
                    {(group) => (
                      <ExchangeGroupPanel
                        data={props.data}
                        group={group}
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                        onInspectItem={props.onInspectItem}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={materialPlan().gathering.length > 0}>
              <section>
                <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                  <Leaf class="h-4 w-4 text-emerald-700" />
                  采集清单
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().gathering}>
                    {(entry) => (
                      <MaterialPlanRow
                        data={props.data}
                        entry={entry}
                        class="border-l-emerald-200 bg-emerald-50/80"
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={materialPlan().shops.length > 0}>
              <section>
                <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                  <Coins class="h-4 w-4 text-amber-700" />
                  商店购买
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().shops}>
                    {(entry) => (
                      <MaterialPlanRow
                        data={props.data}
                        entry={entry}
                        class="border-l-amber-200 bg-amber-50/80"
                        meta={`${entry.shopName}${entry.gil ? ` · ${formatInteger(entry.gil)}G` : ''}`}
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={materialPlan().market.length > 0}>
              <section>
                <div class="mb-2 flex items-center justify-between gap-2 text-sm font-semibold">
                  <div class="flex items-center gap-2">
                    <Coins class="h-4 w-4 text-[#1d4ed8]" />
                    市场购买
                  </div>
                  <div class="text-xs font-medium text-muted-foreground">
                    <Show when={marketQuotes.loading} fallback={marketCost() > 0 ? `估 ${formatInteger(marketCost())}G` : `区域 ${MARKET_WORLD_DC_REGION}`}>
                      估价载入中
                    </Show>
                  </div>
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().market}>
                    {(entry) => (
                      <MaterialPlanRow
                        data={props.data}
                        entry={entry}
                        class="border-l-[#93c5fd] bg-[#eff6ff]"
                        meta={marketMeta(entry)}
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={materialPlan().unknown.length > 0}>
              <section>
                <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                  <PackageSearch class="h-4 w-4 text-muted-foreground" />
                  未安排
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().unknown}>
                    {(entry) => (
                      <MaterialPlanRow
                        data={props.data}
                        entry={entry}
                        class="border-l-border bg-background"
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>

            <Show when={materialPlan().owned.length > 0}>
              <section>
                <div class="mb-2 flex items-center gap-2 text-sm font-semibold text-muted-foreground">
                  <CircleCheck class="h-4 w-4" />
                  已拥有
                </div>
                <div class="space-y-2">
                  <For each={materialPlan().owned}>
                    {(entry) => (
                      <MaterialPlanRow
                        data={props.data}
                        entry={entry}
                        class="border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground"
                        onChoose={props.onChoose}
                        onInspect={inspectEntry}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Show>
          </div>
        </Show>
      </div>
    </section>
  )
}

function MergedCraftGraphNodeCard(props: {
  data: CraftDataPackage
  node: PositionedCraftGraphNode
  width: number
  height: number
  sourceChoices: Map<number, SourceChoice>
  onToggle: (itemId: number, collapsed: boolean) => void
  onSelect: (node: PositionedCraftGraphNode) => void
}) {
  const item = () => getItem(props.data, props.node.itemId)
  const countsAsLeaf = () => !props.node.craftable || props.node.collapsed
  const toneClass = () => countsAsLeaf()
    ? leafToneClass(
      props.data,
      {
        itemId: props.node.itemId,
        amountNeeded: props.node.amount,
        recipe: props.node.recipe,
        children: [],
      },
      props.sourceChoices,
    )
    : 'border-l-transparent bg-background hover:bg-accent/70'

  const subtitle = () => {
    const recipe = props.node.recipe
    return recipe ? `${CRAFT_TYPE_ABBRS[Math.min(recipe.craftType, 7)]} · ${recipeLevelLabel(props.data, recipe)}` : undefined
  }

  return (
    <div
      class="absolute"
      style={{ left: `${props.node.x}px`, top: `${props.node.y}px`, width: `${props.width}px`, height: `${props.height}px` }}
    >
      <button
        type="button"
        class={cx(
          'grid h-full w-full grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-1.5 rounded border border-border border-l-2 px-2 py-1.5 text-left text-xs shadow-sm transition-colors',
          toneClass(),
        )}
        onClick={() => {
          props.onSelect(props.node)
        }}
      >
        <ItemIcon icon={item()?.icon ?? 0} size="sm" />
        <div class="min-w-0">
          <div class="truncate font-medium">{getItemName(props.data, props.node.itemId)}</div>
          <Show when={subtitle()}>
            {(value) => (
              <div class="truncate text-[11px] text-muted-foreground" title={value()}>
                {value()}
              </div>
            )}
          </Show>
        </div>
        <div class="flex flex-col items-end gap-1">
          <Badge variant="secondary" class="h-4 px-1 text-[10px]">总 {formatInteger(props.node.amount)}</Badge>
          <Show when={props.node.root}>
            <Badge variant="outline" class="h-4 bg-background/80 px-1 text-[10px]">目标</Badge>
          </Show>
          <Show when={countsAsLeaf() && !props.node.root}>
            <Badge variant="outline" class="h-4 bg-background/80 px-1 text-[10px]">叶</Badge>
          </Show>
        </div>
      </button>

      <Show when={props.node.craftable}>
        <button
          type="button"
          class="absolute right-[-12px] top-1/2 z-10 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-full border bg-background text-muted-foreground shadow-sm transition-colors hover:bg-accent hover:text-foreground"
          aria-label={props.node.collapsed ? '继续分解' : '停止分解'}
          title={props.node.collapsed ? '继续分解' : '停止分解'}
          onClick={(event) => {
            event.stopPropagation()
            props.onToggle(props.node.itemId, !props.node.collapsed)
          }}
        >
          <Show when={props.node.collapsed} fallback={<ChevronRight class="h-3.5 w-3.5" />}>
            <Plus class="h-3.5 w-3.5" />
          </Show>
        </button>
      </Show>
    </div>
  )
}

function CrystalCostMatrix(props: { rows: CrystalMatrixRow[] }) {
  const total = () => props.rows.reduce((sum, row) => sum + row.total, 0)
  const hasUsage = () => crystalMatrixHasUsage(props.rows)

  return (
    <div class="overflow-hidden rounded-md border bg-[#fafaf9]">
      <div class="flex flex-wrap items-center justify-between gap-2 p-3 pb-2">
        <div>
          <div class="text-xs font-semibold text-foreground">晶石消耗</div>
          <div class="mt-0.5 text-[11px] text-muted-foreground">碎晶 / 水晶 / 晶簇按元素汇总，图中不再显示为节点</div>
        </div>
        <div class="flex items-center gap-2">
          <Show when={!hasUsage()}>
            <Badge variant="outline" class="bg-background/80">暂无消耗</Badge>
          </Show>
          <Badge variant="secondary">总 {formatInteger(total())}</Badge>
        </div>
      </div>

      <div class="overflow-x-auto px-3 pb-3">
        <div class="min-w-[620px] overflow-hidden rounded-md border bg-background">
          <div class="grid grid-cols-[4.5rem_repeat(6,minmax(4.25rem,1fr))] border-b bg-muted/30 text-[11px] font-medium text-muted-foreground">
            <div class="px-2 py-1.5">类型</div>
            <For each={CRYSTAL_ELEMENTS}>
              {(element) => (
                <div class="flex items-center justify-end gap-1.5 px-2 py-1.5">
                  <span
                    class="h-2 w-2 rounded-full"
                    style={{ 'background-color': CRYSTAL_ELEMENT_COLORS[element] }}
                  />
                  <span>{element}</span>
                </div>
              )}
            </For>
          </div>
          <For each={props.rows}>
            {(row) => (
              <div class="grid grid-cols-[4.5rem_repeat(6,minmax(4.25rem,1fr))] border-b last:border-b-0">
                <div class="flex items-center justify-between gap-2 bg-muted/20 px-2 py-2 text-xs font-medium">
                  <span>{row.tier}</span>
                  <span class={cx('text-[11px]', row.total > 0 ? 'text-foreground' : 'text-muted-foreground/45')}>
                    {formatInteger(row.total)}
                  </span>
                </div>
                <For each={row.cells}>
                  {(cell) => (
                    <div class="flex items-center justify-end border-l px-2 py-2">
                      <span
                        class={cx(
                          'tabular-nums',
                          cell.amount > 0 ? 'text-sm font-semibold text-foreground' : 'text-xs text-muted-foreground/35',
                        )}
                      >
                        {formatInteger(cell.amount)}
                      </span>
                    </div>
                  )}
                </For>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  )
}

function CraftSummaryGraph(props: {
  data: CraftDataPackage
  roots: CraftGraphRoot[]
  sourceChoices: Map<number, SourceChoice>
  onToggleCollapsedItem: (itemId: number, collapsed: boolean) => void
  onSelect: (target: DetailTarget) => void
}) {
  const [zoom, setZoom] = createSignal(1)
  const graph = createMemo(() => buildMergedCraftGraph(props.data, props.roots))
  const layout = createMemo(() => {
    const nodeWidth = 184
    const nodeHeight = 58
    const xGap = 116
    const padding = 18
    const lanes = orderedGraphLanes(graph().nodes, graph().edges)
    const depths = [...lanes.keys()].sort((a, b) => a - b)
    const laneLayout = adaptiveGraphCenters(lanes, graph().edges, nodeHeight)

    const positioned: PositionedCraftGraphNode[] = []
    depths.forEach((depth, column) => {
      const lane = lanes.get(depth) ?? []
      lane.forEach((node, index) => {
        positioned.push({
          ...node,
          x: padding + column * (nodeWidth + xGap),
          y: padding + (laneLayout.centers.get(node.itemId) ?? initialLaneCenter(lane.length, index, laneLayout.contentHeight, nodeHeight)) - nodeHeight / 2,
        })
      })
    })

    const byItemId = new Map(positioned.map((node) => [node.itemId, node]))
    const positionedEdges: PositionedCraftGraphEdge[] = []
    for (const edge of graph().edges) {
      const fromNode = byItemId.get(edge.from)
      const toNode = byItemId.get(edge.to)
      if (fromNode && toNode) positionedEdges.push({ ...edge, fromNode, toNode, fromOffset: 0, toOffset: 0 })
    }
    applyGraphEdgePorts(positionedEdges, nodeHeight)

    const maxX = positioned.reduce((value, node) => Math.max(value, node.x + nodeWidth), nodeWidth)
    const maxY = positioned.reduce((value, node) => Math.max(value, node.y + nodeHeight), nodeHeight)

    return {
      nodes: positioned,
      edges: positionedEdges,
      width: maxX + padding,
      height: Math.max(maxY + padding, laneLayout.contentHeight + padding * 2),
      nodeWidth,
      nodeHeight,
    }
  })
  const zoomPercent = () => `${Math.round(zoom() * 100)}%`
  const updateZoom = (next: number) => setZoom(Math.min(1.4, Math.max(0.5, Math.round(next * 100) / 100)))

  return (
    <section class="overflow-hidden rounded-md border bg-background">
      <div class="flex flex-wrap items-center justify-between gap-3 border-b p-3">
        <div class="min-w-0">
          <div class="text-sm font-semibold">合成图</div>
          <div class="mt-0.5 text-xs text-muted-foreground">全部目标合并为一张图，相同物品共用节点</div>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <Badge variant="outline">目标 {props.roots.length}</Badge>
          <Badge variant="outline">节点 {layout().nodes.length}</Badge>
          <Badge variant="outline">边 {layout().edges.length}</Badge>
          <div class="flex items-center gap-1 rounded-md border bg-background p-1">
            <button
              type="button"
              class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
              title="缩小"
              aria-label="缩小"
              onClick={() => updateZoom(zoom() - 0.1)}
            >
              <ZoomOut class="h-4 w-4" />
            </button>
            <div class="w-12 text-center text-xs font-medium text-muted-foreground">{zoomPercent()}</div>
            <button
              type="button"
              class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
              title="放大"
              aria-label="放大"
              onClick={() => updateZoom(zoom() + 0.1)}
            >
              <ZoomIn class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
              title="重置缩放"
              aria-label="重置缩放"
              onClick={() => updateZoom(1)}
            >
              <RotateCcw class="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>

      <div class="overflow-x-auto overflow-y-visible bg-muted/10 p-3">
        <div
          class="relative min-w-max rounded-md border bg-background"
          style={{
            width: `${layout().width * zoom()}px`,
            height: `${layout().height * zoom()}px`,
          }}
        >
          <div
            class="absolute left-0 top-0 origin-top-left"
            style={{
              width: `${layout().width}px`,
              height: `${layout().height}px`,
              transform: `scale(${zoom()})`,
            }}
          >
            <svg
              class="pointer-events-none absolute inset-0"
              width={layout().width}
              height={layout().height}
              viewBox={`0 0 ${layout().width} ${layout().height}`}
            >
              <defs>
                <marker id="craft-graph-arrow" markerWidth="10" markerHeight="10" refX="9" refY="5" orient="auto">
                  <path d="M0,0 L10,5 L0,10 Z" fill={GRAPH_EDGE_COLOR} />
                </marker>
              </defs>
              <For each={layout().edges}>
                {(edge) => {
                  const x1 = edge.fromNode.x + layout().nodeWidth
                  const y1 = edge.fromNode.y + layout().nodeHeight / 2 + edge.fromOffset
                  const x2 = edge.toNode.x
                  const y2 = edge.toNode.y + layout().nodeHeight / 2 + edge.toOffset
                  const midX = (x1 + x2) / 2
                  const path = `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`

                  return (
                    <g>
                      <path d={path} fill="none" stroke={GRAPH_EDGE_COLOR} stroke-opacity="0.68" stroke-width="1.75" marker-end="url(#craft-graph-arrow)" />
                      <foreignObject x={x1 + (x2 - x1) * 0.68 - 22} y={y1 + (y2 - y1) * 0.68 - 10} width="44" height="20">
                        <div class="flex h-5 items-center justify-center">
                          <span class="rounded border bg-background/90 px-1 text-[9px] font-medium leading-none text-muted-foreground shadow-sm">
                            x{formatInteger(edge.amount)}
                          </span>
                        </div>
                      </foreignObject>
                    </g>
                  )
                }}
              </For>
            </svg>

            <For each={layout().nodes}>
              {(node) => (
                <MergedCraftGraphNodeCard
                  data={props.data}
                  node={node}
                  width={layout().nodeWidth}
                  height={layout().nodeHeight}
                  sourceChoices={props.sourceChoices}
                  onToggle={props.onToggleCollapsedItem}
                  onSelect={(value) => props.onSelect({
                    itemId: value.itemId,
                    amountNeeded: value.amount,
                    recipe: value.recipe,
                  })}
                />
              )}
            </For>

            <Show when={layout().nodes.length === 0}>
              <div class="absolute inset-0 flex items-center justify-center">
                <EmptyState icon={<PackageSearch class="h-6 w-6" />} title="暂无图节点" />
              </div>
            </Show>
          </div>
        </div>
      </div>
    </section>
  )
}

function CraftSummaryCardView(props: {
  data: CraftDataPackage
  engine: CraftDataEngine
  card: CraftSummaryCard
  recipes: Map<number, CraftRecipe>
  active: boolean
  sourceChoices: Map<number, SourceChoice>
  onSelect: () => void
  onEdit: () => void
  onRemove: () => void
  onToggleCollapsedItem: (itemId: number, collapsed: boolean) => void
  onChooseSource: (itemId: number, choice: SourceChoice | undefined) => void
  onInspect: (target: DetailTarget) => void
}) {
  const materials = createMemo(() => summarizeCardMaterials(props.engine, props.card))
  const graphRoots = createMemo(() => props.card.targets.map((target) => {
    const amount = Math.max(1, Math.floor(target.amount || 1))
    const tree = buildCraftTree(props.engine, target.itemId, amount)
    return {
      target,
      tree,
      collapsed: new Set(target.collapsed),
      recipe: props.recipes.get(target.recipeId) ?? tree.recipe,
    }
  }))

  return (
    <div
      class={cx(
        'w-full min-w-0 overflow-hidden rounded-md border bg-card transition-colors',
        props.active && 'border-foreground/30 ring-1 ring-foreground/10',
      )}
      onClick={props.onSelect}
    >
      <div class="flex items-start gap-3 border-b p-3">
        <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border bg-background text-muted-foreground">
          <PackageSearch class="h-4 w-4" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-sm font-semibold">{props.card.title}</div>
          <div class="mt-0.5 flex flex-wrap gap-1.5 text-xs text-muted-foreground">
            <Badge variant="outline">物品 {props.card.targets.length}</Badge>
            <Badge variant="outline">叶子 {materials().length}</Badge>
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <button
            type="button"
            class="flex h-8 w-8 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            title="编辑卡片"
            aria-label="编辑卡片"
            onClick={(event) => {
              event.stopPropagation()
              props.onEdit()
            }}
          >
            <Pencil class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="flex h-8 w-8 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            title="删除卡片"
            aria-label="删除卡片"
            onClick={(event) => {
              event.stopPropagation()
              props.onRemove()
            }}
          >
            <Trash2 class="h-4 w-4" />
          </button>
        </div>
      </div>

      <div class="space-y-3 p-3">
        <Show
          when={graphRoots().length > 0}
          fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="暂无物品" action={<Button size="sm" variant="outline" onClick={props.onEdit}><Plus class="h-4 w-4" />选择物品</Button>} />}
        >
          <CraftSummaryGraph
            data={props.data}
            roots={graphRoots()}
            sourceChoices={props.sourceChoices}
            onToggleCollapsedItem={props.onToggleCollapsedItem}
            onSelect={props.onInspect}
          />
        </Show>

        <MaterialSummaryPanel
          data={props.data}
          title={props.card.title}
          materials={materials()}
          sourceChoices={props.sourceChoices}
          onChoose={props.onChooseSource}
          onInspectItem={(itemId, amountNeeded) => props.onInspect({ itemId, amountNeeded })}
        />
      </div>
    </div>
  )
}

function TreeNodeRow(props: {
  node: NoteTreeNode
  depth: number
  activePageId?: string
  expanded: Set<string>
  onToggle: (id: string) => void
  onSelectPage: (id: string) => void
  onAddFolder: (parentId: string) => void
  onAddPage: (parentId: string) => void
  onRename: (id: string, current: string) => void
  onDelete: (id: string) => void
}) {
  const isFolder = () => props.node.kind === 'folder'
  const isExpanded = () => props.expanded.has(props.node.id)
  const active = () => props.node.kind === 'page' && props.activePageId === props.node.id

  return (
    <div>
      <div
        class={cx(
          'group flex h-9 items-center gap-1 rounded-md px-2 text-sm transition-colors',
          active() ? 'bg-accent text-foreground' : 'text-muted-foreground hover:bg-accent/70 hover:text-foreground',
        )}
        style={{ 'padding-left': `${8 + props.depth * 16}px` }}
      >
        <button
          type="button"
          class="flex h-6 w-6 shrink-0 items-center justify-center rounded hover:bg-background/80"
          aria-label={isFolder() ? (isExpanded() ? '折叠目录' : '展开目录') : '打开页面'}
          onClick={() => {
            if (isFolder()) props.onToggle(props.node.id)
            else props.onSelectPage(props.node.id)
          }}
        >
          <Show when={isFolder()} fallback={<BookOpen class="h-4 w-4" />}>
            <Show when={isExpanded()} fallback={<ChevronRight class="h-4 w-4" />}>
              <ChevronDown class="h-4 w-4" />
            </Show>
          </Show>
        </button>
        <button
          type="button"
          class="min-w-0 flex-1 truncate text-left"
          onClick={() => {
            if (isFolder()) props.onToggle(props.node.id)
            else props.onSelectPage(props.node.id)
          }}
        >
          {props.node.title}
        </button>
        <div class="flex shrink-0 items-center gap-0.5 opacity-70 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          <Show when={isFolder()}>
            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background"
              title="添加页面"
              aria-label="添加页面"
              onClick={(event) => {
                event.stopPropagation()
                props.onAddPage(props.node.id)
              }}
            >
              <FilePlus2 class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background"
              title="添加目录"
              aria-label="添加目录"
              onClick={(event) => {
                event.stopPropagation()
                props.onAddFolder(props.node.id)
              }}
            >
              <FolderPlus class="h-3.5 w-3.5" />
            </button>
          </Show>
          <button
            type="button"
            class="flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background"
            title="重命名"
            aria-label="重命名"
            onClick={(event) => {
              event.stopPropagation()
              props.onRename(props.node.id, props.node.title)
            }}
          >
            <MoreHorizontal class="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            class="flex h-6 w-6 items-center justify-center rounded border border-transparent bg-background/60 hover:border-border hover:bg-background"
            title="删除"
            aria-label="删除"
            onClick={(event) => {
              event.stopPropagation()
              props.onDelete(props.node.id)
            }}
          >
            <Trash2 class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <Show when={isFolder() && isExpanded()}>
        <For each={props.node.children ?? []}>
          {(child) => (
            <TreeNodeRow
              node={child}
              depth={props.depth + 1}
              activePageId={props.activePageId}
              expanded={props.expanded}
              onToggle={props.onToggle}
              onSelectPage={props.onSelectPage}
              onAddFolder={props.onAddFolder}
              onAddPage={props.onAddPage}
              onRename={props.onRename}
              onDelete={props.onDelete}
            />
          )}
        </For>
      </Show>
    </div>
  )
}

function NameDialog(props: NameDialogState & { onClose: () => void }) {
  const [value, setValue] = createSignal(props.initialValue)
  const trimmed = () => value().trim()

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      role="dialog"
      aria-modal="true"
      onClick={props.onClose}
    >
      <form
        class="w-full max-w-sm overflow-hidden rounded-md border bg-card shadow-xl"
        onClick={(event) => event.stopPropagation()}
        onSubmit={(event) => {
          event.preventDefault()
          if (!trimmed()) return
          props.onConfirm(trimmed())
          props.onClose()
        }}
      >
        <div class="flex items-center justify-between gap-3 border-b p-4">
          <div class="min-w-0 text-base font-semibold">{props.title}</div>
          <button
            type="button"
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭"
            title="关闭"
            onClick={props.onClose}
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="space-y-3 p-4">
          <label class="grid gap-1.5 text-sm font-medium">
            {props.label}
            <Input
              value={value()}
              autofocus
              onInput={(event) => setValue(event.currentTarget.value)}
            />
          </label>
        </div>

        <div class="flex justify-end gap-2 border-t bg-muted/30 p-3">
          <Button type="button" variant="outline" onClick={props.onClose}>取消</Button>
          <Button type="submit" variant="primary" disabled={!trimmed()}>{props.confirmLabel}</Button>
        </div>
      </form>
    </div>
  )
}

function ConfirmDialog(props: ConfirmDialogState & { onClose: () => void }) {
  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      role="dialog"
      aria-modal="true"
      onClick={props.onClose}
    >
      <div
        class="w-full max-w-sm overflow-hidden rounded-md border bg-card shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div class="flex items-center justify-between gap-3 border-b p-4">
          <div class="min-w-0 text-base font-semibold">{props.title}</div>
          <button
            type="button"
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭"
            title="关闭"
            onClick={props.onClose}
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="p-4 text-sm text-muted-foreground">{props.description}</div>

        <div class="flex justify-end gap-2 border-t bg-muted/30 p-3">
          <Button type="button" variant="outline" onClick={props.onClose}>取消</Button>
          <button
            type="button"
            class="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md bg-destructive px-3 text-sm font-medium text-destructive-foreground transition-colors hover:bg-destructive/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onClick={() => {
              props.onConfirm()
              props.onClose()
            }}
          >
            {props.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}

function CraftSummaryEditorDialog(props: {
  data: CraftDataPackage
  engine: CraftDataEngine
  card?: CraftSummaryCard
  onSave: (card: CraftSummaryCard) => void
  onClose: () => void
}) {
  const [query, setQuery] = createSignal('')
  const [craftType, setCraftType] = createSignal<number | undefined>()
  const [title, setTitle] = createSignal(props.card?.title ?? '合成汇总')
  const [targets, setTargets] = createSignal<CraftSummaryTarget[]>(props.card?.targets.map((target) => ({ ...target, collapsed: [...target.collapsed] })) ?? [])
  const recipes = createMemo(() => craftableRecipes(props.engine, craftType(), query(), 120))
  const selectedRecipeIds = createMemo(() => new Set(targets().map((target) => target.recipeId)))

  const addRecipe = (recipe: CraftRecipe) => {
    setTargets((current) => {
      if (current.some((target) => target.recipeId === recipe.id)) return current
      return [...current, {
        id: id(),
        recipeId: recipe.id,
        itemId: recipe.resultItemId,
        amount: 1,
        collapsed: [],
      }]
    })
  }
  const removeTarget = (targetId: string) => {
    setTargets((current) => current.filter((target) => target.id !== targetId))
  }
  const updateTargetAmount = (targetId: string, amount: number) => {
    setTargets((current) => current.map((target) => (
      target.id === targetId ? { ...target, amount: Math.max(1, Math.floor(amount || 1)) } : target
    )))
  }
  const save = () => {
    const trimmedTitle = title().trim() || '合成汇总'
    props.onSave({
      id: props.card?.id ?? id(),
      kind: 'craftSummary',
      title: trimmedTitle,
      targets: targets(),
      sourceChoices: props.card?.sourceChoices ?? {},
    })
    props.onClose()
  }

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      role="dialog"
      aria-modal="true"
      onClick={props.onClose}
    >
      <div
        class="flex max-h-[min(820px,calc(100vh-2rem))] w-full max-w-5xl flex-col overflow-hidden rounded-md border bg-card shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div class="flex items-center gap-3 border-b p-4">
          <div class="flex h-9 w-9 items-center justify-center rounded-md border bg-background">
            <PackageSearch class="h-4 w-4" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="text-base font-semibold">{props.card ? '编辑合成汇总卡片' : '新建合成汇总卡片'}</div>
            <div class="text-xs text-muted-foreground">搜索并多选需要制作的物品</div>
          </div>
          <button
            type="button"
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭"
            title="关闭"
            onClick={props.onClose}
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="grid min-h-0 flex-1 lg:grid-cols-[minmax(0,1fr)_360px]">
          <section class="flex min-h-0 flex-col border-b lg:border-b-0 lg:border-r">
            <div class="space-y-3 border-b p-3">
              <label class="grid gap-1.5 text-sm font-medium">
                卡片名称
                <Input value={title()} onInput={(event) => setTitle(event.currentTarget.value)} />
              </label>
              <div class="relative">
                <Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={query()}
                  onInput={(event) => setQuery(event.currentTarget.value)}
                  placeholder="搜索物品或 ID"
                  class="pl-9 pr-9"
                  autofocus
                />
                <Show when={query()}>
                  <button
                    type="button"
                    class="absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded text-muted-foreground hover:bg-accent"
                    onClick={() => setQuery('')}
                    aria-label="清除搜索"
                    title="清除搜索"
                  >
                    <X class="h-4 w-4" />
                  </button>
                </Show>
              </div>

              <div class="flex flex-wrap gap-1.5">
                <Button size="sm" variant={craftType() == null ? 'primary' : 'outline'} onClick={() => setCraftType(undefined)}>
                  全部
                </Button>
                <For each={CRAFT_TYPE_ABBRS}>
                  {(label, index) => (
                    <Button
                      size="sm"
                      variant={craftType() === index() ? 'primary' : 'outline'}
                      onClick={() => setCraftType(index())}
                    >
                      {label}
                    </Button>
                  )}
                </For>
              </div>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-2">
              <Show
                when={recipes().length > 0}
                fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="没有匹配的配方" />}
              >
                <For each={recipes()}>
                  {(recipe) => {
                    const item = () => getItem(props.data, recipe.resultItemId)
                    const selected = () => selectedRecipeIds().has(recipe.id)
                    return (
                      <button
                        type="button"
                        class={cx(
                          'mb-1 grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors',
                          selected() ? 'bg-accent text-foreground' : 'hover:bg-accent/70',
                        )}
                        onClick={() => addRecipe(recipe)}
                      >
                        <ItemIcon icon={item()?.icon ?? 0} />
                        <div class="min-w-0">
                          <div class="truncate font-medium">{getItemName(props.data, recipe.resultItemId)}</div>
                          <div class="truncate text-xs text-muted-foreground">
                            {CRAFT_TYPE_NAMES[Math.min(recipe.craftType, 7)]} · {recipeLevelLabel(props.data, recipe)}
                          </div>
                        </div>
                        <Badge variant={selected() ? 'secondary' : 'outline'}>
                          {selected() ? '已选' : `#${recipe.resultItemId}`}
                        </Badge>
                      </button>
                    )
                  }}
                </For>
              </Show>
            </div>
          </section>

          <aside class="flex min-h-0 flex-col">
            <div class="flex items-center justify-between gap-3 border-b p-3">
              <div class="text-sm font-semibold">已选物品</div>
              <Badge variant="outline">{targets().length}</Badge>
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto p-3">
              <Show
                when={targets().length > 0}
                fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="还没有选择物品" />}
              >
                <div class="space-y-2">
                  <For each={targets()}>
                    {(target) => {
                      const item = () => getItem(props.data, target.itemId)
                      const recipe = () => props.data.recipes.find((item) => item.id === target.recipeId)
                      return (
                        <div class="rounded-md border bg-background p-2">
                          <div class="grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-2">
                            <ItemIcon icon={item()?.icon ?? 0} size="sm" />
                            <div class="min-w-0">
                              <div class="truncate text-sm font-medium">{getItemName(props.data, target.itemId)}</div>
                              <Show when={recipe()}>
                                {(value) => <div class="truncate text-xs text-muted-foreground">{CRAFT_TYPE_NAMES[Math.min(value().craftType, 7)]}</div>}
                              </Show>
                            </div>
                            <button
                              type="button"
                              class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                              title="移除"
                              aria-label="移除"
                              onClick={() => removeTarget(target.id)}
                            >
                              <X class="h-4 w-4" />
                            </button>
                          </div>
                          <label class="mt-2 grid gap-1 text-xs font-medium text-muted-foreground">
                            数量
                            <Input
                              type="number"
                              min={1}
                              value={target.amount}
                              class="h-8"
                              onInput={(event) => updateTargetAmount(target.id, Number(event.currentTarget.value))}
                            />
                          </label>
                        </div>
                      )
                    }}
                  </For>
                </div>
              </Show>
            </div>
          </aside>
        </div>

        <div class="flex justify-end gap-2 border-t bg-muted/30 p-3">
          <Button type="button" variant="outline" onClick={props.onClose}>取消</Button>
          <Button type="button" variant="primary" disabled={targets().length === 0} onClick={save}>保存卡片</Button>
        </div>
      </div>
    </div>
  )
}

function ItemDetailDialog(props: {
  data: CraftDataPackage
  target: DetailTarget
  recipe?: CraftRecipe
  onClose: () => void
}) {
  const item = () => getItem(props.data, props.target.itemId)
  const sources = () => props.data.sources[String(props.target.itemId)] ?? []
  const sourceGroups = () => sourceDisplayGroups(props.data, props.target.itemId, sources(), props.target.amountNeeded)

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      role="dialog"
      aria-modal="true"
      onClick={props.onClose}
    >
      <div
        class="max-h-[min(680px,calc(100vh-2rem))] w-full max-w-md overflow-hidden rounded-md border bg-card shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div class="flex items-start gap-3 border-b p-4">
          <ItemIcon icon={item()?.icon ?? 0} />
          <div class="min-w-0 flex-1">
            <div class="truncate text-base font-semibold">{getItemName(props.data, props.target.itemId)}</div>
            <div class="text-xs text-muted-foreground">#{props.target.itemId}</div>
          </div>
          <button
            type="button"
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭"
            title="关闭"
            onClick={props.onClose}
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="max-h-[calc(100vh-8rem)] overflow-y-auto p-4">
          <div class="space-y-4">
            <ItemExternalLinks
              item={item()}
              itemId={props.target.itemId}
              itemName={getItemName(props.data, props.target.itemId)}
              sources={sources()}
            />

            <div class="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
              <div class="text-muted-foreground">需求</div>
              <div class="text-right font-medium">x{formatInteger(props.target.amountNeeded)}</div>
              <Show when={item()?.priceLow}>
                <div class="text-muted-foreground">收购</div>
                <div class="text-right font-medium">{formatInteger(item()!.priceLow)}G</div>
              </Show>
              <Show when={props.recipe}>
                {(value) => (
                  <>
                    <div class="text-muted-foreground">职业</div>
                    <div class="text-right font-medium">{CRAFT_TYPE_NAMES[Math.min(value().craftType, 7)]}</div>
                    <div class="text-muted-foreground">等级</div>
                    <div class="text-right font-medium">{recipeLevelLabel(props.data, value())}</div>
                    <div class="text-muted-foreground">产出</div>
                    <div class="text-right font-medium">x{value().resultAmount}</div>
                  </>
                )}
              </Show>
            </div>

            <Show when={sourceGroups().length > 0}>
              <div class="space-y-2">
                <div class="text-sm font-medium">获取来源</div>
                <For each={sourceGroups()}>
                  {(group) => (
                    <div class={cx('flex items-start gap-2 rounded-sm border-l-2 p-2 text-sm', sourceToneClass(group.source))}>
                      <div class="mt-0.5 text-muted-foreground">{sourceIcon(group.source)}</div>
                      <div class="min-w-0 flex-1">
                        <div class="font-medium">
                          {sourceLabel(group.source)}
                          <Show when={group.costLabel}>
                            {(label) => <span class="ml-1 text-muted-foreground">{label()}</span>}
                          </Show>
                        </div>
                        <div class="whitespace-normal break-words text-xs leading-snug text-muted-foreground">
                          <Show when={group.details.length > 1} fallback={group.details[0]}>
                            <ul class="list-disc space-y-0.5 pl-4">
                              <For each={group.details}>
                                {(detail) => <li>{detail}</li>}
                              </For>
                            </ul>
                          </Show>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </div>
  )
}
