import initWasm, {
  CraftDataEngine as WasmCraftDataEngine,
  defaultSourceIndex as wasmDefaultSourceIndex,
  resolveSource as wasmResolveSource,
  sourceLabel as wasmSourceLabel,
  sourcePriority as wasmSourcePriority,
  summarizeMaterials as wasmSummarizeMaterials,
} from '../wasm/xiv_companion'
import type {
  CraftDataPackage,
  CraftItem,
  CraftRecipe,
  CraftTreeNode,
  ItemSource,
  MaterialSummary,
  RecipeLevelInfo,
  SourceChoiceEntry,
  SourceChoice,
  SpecialShopCost,
} from '../wasm/xiv_companion'
import { baseUrl } from './utils'

declare const __BUILD_COMMIT__: string
declare const __BUILD_DATE__: string

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

export type {
  CraftDataPackage,
  CraftItem,
  CraftRecipe,
  CraftTreeNode,
  ItemSource,
  MaterialSummary,
  RecipeLevelInfo,
  SourceChoice,
  SpecialShopCost,
}

export type CraftDataEngine = WasmCraftDataEngine

let craftDataPromise: Promise<CraftDataPackage> | null = null
let wasmPromise: Promise<void> | null = null

export async function loadCraftData(): Promise<CraftDataPackage> {
  if (craftDataPromise) return craftDataPromise
  craftDataPromise = fetch(`${baseUrl()}craft-data.json?v=${__BUILD_COMMIT__}-${encodeURIComponent(__BUILD_DATE__)}`)
    .then((res) => {
      if (!res.ok) throw new Error(`craft-data.json ${res.status}`)
      return res.json() as Promise<CraftDataPackage>
    })
  return craftDataPromise
}

export async function createCraftDataEngine(data: CraftDataPackage): Promise<CraftDataEngine> {
  await initCraftWasm()
  return new WasmCraftDataEngine(data)
}

export function craftableRecipes(
  engine: CraftDataEngine,
  craftType: number | undefined,
  query: string,
  limit = 300,
): CraftRecipe[] {
  return engine.craftableRecipes(craftType, query, limit)
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
  engine: CraftDataEngine,
  itemId: number,
  amount: number,
): CraftTreeNode {
  return engine.buildCraftTree(itemId, amount)
}

export function summarizeMaterials(
  node: CraftTreeNode,
  collapsed = new Set<string>(),
): MaterialSummary[] {
  return wasmSummarizeMaterials(node, [...collapsed])
}

export function collapseKey(itemId: number, depth: number): string {
  return `${itemId}:${depth}`
}

export function defaultSourceIndex(sources: ItemSource[]): number | undefined {
  return wasmDefaultSourceIndex(sources) ?? undefined
}

export function resolveSource(
  itemId: number,
  sources: ItemSource[],
  choices: Map<number, SourceChoice>,
): ItemSource | undefined {
  return wasmResolveSource(itemId, sources, sourceChoiceEntries(choices)) ?? undefined
}

export function sourceLabel(source: ItemSource): string {
  return wasmSourceLabel(source)
}

export function sourcePriority(source: ItemSource): number {
  return wasmSourcePriority(source)
}

function initCraftWasm() {
  wasmPromise ??= initWasm().then(() => undefined)
  return wasmPromise
}

function sourceChoiceEntries(choices: Map<number, SourceChoice>): SourceChoiceEntry[] {
  return [...choices.entries()].map(([itemId, choice]) => ({ itemId, choice }))
}
