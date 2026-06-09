import { baseUrl } from './utils'

declare const __BUILD_COMMIT__: string

export const CRAFT_TYPE_NAMES = [
  '刻木匠',
  '锻铁匠',
  '铸甲匠',
  '雕金匠',
  '制革匠',
  '裁衣匠',
  '炼金术士',
  '烹调师',
] as const

export const CRAFT_TYPE_ABBRS = [
  '木工',
  '锻冶',
  '甲胄',
  '雕金',
  '皮革',
  '裁缝',
  '炼金',
  '烹调',
] as const

export interface CraftItem {
  id: number
  name: string
  icon: number
  itemUiCategory: number
  itemSearchCategory: number
  priceMid: number
  priceLow: number
}

export interface CraftIngredient {
  itemId: number
  amount: number
}

export interface CraftRecipe {
  id: number
  resultItemId: number
  resultAmount: number
  craftType: number
  recipeLevelTableId: number
  ingredients: CraftIngredient[]
  secretRecipeBook: number
}

export interface RecipeLevelInfo {
  classJobLevel: number
  stars: number
  difficulty: number
  quality: number
  durability: number
}

export interface SpecialShopCost {
  itemId: number
  count: number
}

export type ItemSource =
  | { kind: 'gilShop'; shopName: string }
  | { kind: 'specialShop'; shopName: string; costs: SpecialShopCost[] }
  | { kind: 'gathering' }

export interface CraftDataPackage {
  generatedAt: string
  source: string
  counts: {
    items: number
    recipes: number
    sources: number
  }
  items: Record<string, CraftItem>
  recipes: CraftRecipe[]
  recipeLevels: Record<string, RecipeLevelInfo>
  secretRecipeBooks: Record<string, string>
  sources: Record<string, ItemSource[]>
}

export interface CraftDataIndex {
  recipesByResult: Map<number, CraftRecipe[]>
  craftableByType: Map<number, CraftRecipe[]>
}

export interface CraftTreeNode {
  itemId: number
  amountNeeded: number
  recipe?: CraftRecipe
  children: CraftTreeNode[]
}

export type SourceChoice =
  | { kind: 'index'; index: number }
  | { kind: 'ignore' }

let craftDataPromise: Promise<CraftDataPackage> | null = null

export async function loadCraftData(): Promise<CraftDataPackage> {
  if (craftDataPromise) return craftDataPromise
  craftDataPromise = fetch(`${baseUrl()}craft-data.json?v=${__BUILD_COMMIT__}`)
    .then((res) => {
      if (!res.ok) throw new Error(`craft-data.json ${res.status}`)
      return res.json() as Promise<CraftDataPackage>
    })
  return craftDataPromise
}

export function createCraftDataIndex(data: CraftDataPackage): CraftDataIndex {
  const recipesByResult = new Map<number, CraftRecipe[]>()
  const craftableByType = new Map<number, CraftRecipe[]>()

  for (const recipe of data.recipes) {
    const byResult = recipesByResult.get(recipe.resultItemId) ?? []
    byResult.push(recipe)
    recipesByResult.set(recipe.resultItemId, byResult)

    const craftType = Math.min(Math.max(recipe.craftType, 0), 7)
    const byType = craftableByType.get(craftType) ?? []
    byType.push(recipe)
    craftableByType.set(craftType, byType)
  }

  for (const recipes of craftableByType.values()) {
    recipes.sort((a, b) => a.resultItemId - b.resultItemId)
  }

  return { recipesByResult, craftableByType }
}

export function getItem(data: CraftDataPackage, itemId: number): CraftItem | undefined {
  return data.items[String(itemId)]
}

export function getItemName(data: CraftDataPackage, itemId: number): string {
  return getItem(data, itemId)?.name ?? `物品 #${itemId}`
}

export function getIconUrls(iconId: number): string[] {
  if (!iconId) return []
  const folder = Math.floor(iconId / 1000) * 1000
  const paddedFolder = String(folder).padStart(6, '0')
  const paddedFile = String(iconId).padStart(6, '0')
  return [
    `https://xivapi.com/i/${paddedFolder}/${paddedFile}.png`,
    `https://www.garlandtools.org/files/icons/item/t/${iconId}.png`,
    `https://garlandtools.org/files/icons/item/t/${iconId}.png`,
  ]
}

export function buildCraftTree(
  itemId: number,
  amount: number,
  index: CraftDataIndex,
  visited = new Set<number>(),
): CraftTreeNode {
  const recipe = !visited.has(itemId) ? index.recipesByResult.get(itemId)?.[0] : undefined
  const children: CraftTreeNode[] = []

  if (recipe) {
    visited.add(itemId)
    const craftCount = Math.ceil(amount / Math.max(recipe.resultAmount, 1))
    for (const ingredient of recipe.ingredients) {
      children.push(
        buildCraftTree(
          ingredient.itemId,
          ingredient.amount * craftCount,
          index,
          visited,
        ),
      )
    }
    visited.delete(itemId)
  }

  return { itemId, amountNeeded: amount, recipe, children }
}

export function summarizeMaterials(
  node: CraftTreeNode,
  collapsed = new Set<string>(),
): Array<{ itemId: number; amount: number }> {
  const totals = new Map<number, number>()
  collectLeaves(node, 0, collapsed, totals)
  return [...totals.entries()]
    .map(([itemId, amount]) => ({ itemId, amount }))
    .sort((a, b) => a.itemId - b.itemId)
}

function collectLeaves(
  node: CraftTreeNode,
  depth: number,
  collapsed: Set<string>,
  totals: Map<number, number>,
) {
  const key = collapseKey(node.itemId, depth)
  if (!node.children.length || collapsed.has(key)) {
    totals.set(node.itemId, (totals.get(node.itemId) ?? 0) + node.amountNeeded)
    return
  }

  for (const child of node.children) {
    collectLeaves(child, depth + 1, collapsed, totals)
  }
}

export function collapseKey(itemId: number, depth: number): string {
  return `${itemId}:${depth}`
}

export function defaultSourceIndex(sources: ItemSource[]): number | undefined {
  if (!sources.length) return undefined
  let bestIndex = 0
  let bestPriority = sourcePriority(sources[0]!)
  for (let i = 1; i < sources.length; i += 1) {
    const priority = sourcePriority(sources[i]!)
    if (priority < bestPriority) {
      bestIndex = i
      bestPriority = priority
    }
  }
  return bestIndex
}

export function resolveSource(
  itemId: number,
  sources: ItemSource[],
  choices: Map<number, SourceChoice>,
): ItemSource | undefined {
  const choice = choices.get(itemId)
  if (choice?.kind === 'ignore') return undefined
  if (choice?.kind === 'index') return sources[choice.index]
  const index = defaultSourceIndex(sources)
  return index == null ? undefined : sources[index]
}

export function sourceLabel(source: ItemSource): string {
  if (source.kind === 'gilShop') return '金币商店'
  if (source.kind === 'specialShop') return '兑换'
  return '采集'
}

export function sourcePriority(source: ItemSource): number {
  if (source.kind === 'gathering') return 1
  if (source.kind === 'gilShop') return 2
  return 3
}
