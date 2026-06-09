import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import type {
  CraftDataPackage,
  CraftIngredient,
  CraftItem,
  CraftRecipe,
  ItemSource,
  RecipeLevelInfo,
} from '@xiv-companian/shared'

const ROOT = resolve(import.meta.dir, '..')
const DATAMINING_DIR = resolve(process.env.DATAMINING_DIR ?? join(ROOT, '..', 'ffxiv-datamining-cn'))
const OUT_DIR = join(ROOT, 'app', 'public')

function parseCsvLine(line: string): string[] {
  const fields: string[] = []
  let current = ''
  let quoted = false

  for (let i = 0; i < line.length; i += 1) {
    const char = line[i]
    if (char === '"') {
      if (line[i + 1] === '"') {
        current += '"'
        i += 1
      } else {
        quoted = !quoted
      }
    } else if (char === ',' && !quoted) {
      fields.push(current)
      current = ''
    } else {
      current += char
    }
  }

  fields.push(current)
  return fields
}

function quoteCount(value: string): number {
  let count = 0
  for (let i = 0; i < value.length; i += 1) {
    if (value[i] === '"') {
      if (value[i + 1] === '"') i += 1
      else count += 1
    }
  }
  return count
}

function splitCsv(csv: string): string[] {
  const raw = csv.split(/\r?\n/)
  const lines: string[] = []
  let buffer = ''

  for (const line of raw) {
    if (buffer) buffer += '\n'
    buffer += line
    if (quoteCount(buffer) % 2 === 0) {
      lines.push(buffer)
      buffer = ''
    }
  }

  if (buffer.trim()) lines.push(buffer)
  return lines
}

function readRows(name: string): string[][] {
  const path = join(DATAMINING_DIR, name)
  if (!existsSync(path)) throw new Error(`Missing ${path}`)
  const csv = readFileSync(path, 'utf-8')
  return splitCsv(csv)
    .slice(3)
    .filter((line) => line.trim())
    .map(parseCsvLine)
}

function numberValue(value: string | undefined): number {
  if (!value) return 0
  const n = Number(value)
  return Number.isFinite(n) && n > 0 ? n : 0
}

function keyBase(value: string | undefined): number {
  if (!value) return 0
  const [head] = value.split('.')
  return numberValue(head)
}

function sourceKey(source: ItemSource): string {
  if (source.kind === 'gilShop') return 'gil'
  if (source.kind === 'gathering') return 'gathering'
  return `special:${source.costs.map((cost) => `${cost.itemId}:${cost.count}`).join('+')}`
}

function addSource(map: Record<string, ItemSource[]>, itemId: number, source: ItemSource) {
  if (!itemId) return
  const key = String(itemId)
  const list = map[key] ?? []
  if (!list.some((item) => sourceKey(item) === sourceKey(source))) {
    list.push(source)
  }
  map[key] = list
}

function resolveSpecialShopCostItemId(
  useCurrencyType: number,
  costItemId: number,
): number {
  if (useCurrencyType !== 16) return costItemId

  const currencyItems: Record<number, number> = {
    1: 28, // 亚拉戈诗学神典石
    2: 33913, // 巧手紫票
    4: 33914, // 大地紫票
    6: 41784, // 巧手橙票
    7: 41785, // 大地橙票
  }

  return currencyItems[costItemId] ?? costItemId
}

function loadItems(): Record<string, CraftItem> {
  const items: Record<string, CraftItem> = {}
  for (const row of readRows('Item.csv')) {
    const id = numberValue(row[0])
    const name = row[10] ?? ''
    if (!id || !name) continue
    items[String(id)] = {
      id,
      name,
      icon: numberValue(row[11]),
      itemUiCategory: numberValue(row[16]),
      itemSearchCategory: numberValue(row[17]),
      priceMid: numberValue(row[26]),
      priceLow: numberValue(row[27]),
    }
  }
  return items
}

function loadRecipes(): CraftRecipe[] {
  const recipes: CraftRecipe[] = []
  for (const row of readRows('Recipe.csv')) {
    const id = numberValue(row[0])
    const resultItemId = numberValue(row[5])
    if (!id || !resultItemId) continue

    const ingredients: CraftIngredient[] = []
    for (let i = 0; i < 8; i += 1) {
      const itemId = numberValue(row[7 + i * 2])
      const amount = numberValue(row[8 + i * 2])
      if (itemId && amount) ingredients.push({ itemId, amount })
    }
    if (!ingredients.length) continue

    recipes.push({
      id,
      resultItemId,
      resultAmount: numberValue(row[6]) || 1,
      craftType: numberValue(row[2]),
      recipeLevelTableId: numberValue(row[3]),
      ingredients,
      secretRecipeBook: numberValue(row[41]),
    })
  }
  return recipes
}

function loadRecipeLevels(): Record<string, RecipeLevelInfo> {
  const levels: Record<string, RecipeLevelInfo> = {}
  for (const row of readRows('RecipeLevelTable.csv')) {
    const id = numberValue(row[0])
    if (!id) continue
    levels[String(id)] = {
      classJobLevel: numberValue(row[1]),
      stars: numberValue(row[2]),
      difficulty: numberValue(row[4]),
      quality: numberValue(row[5]),
      durability: numberValue(row[10]),
    }
  }
  return levels
}

function loadSecretRecipeBooks(): Record<string, string> {
  const books: Record<string, string> = {}
  for (const row of readRows('SecretRecipeBook.csv')) {
    const id = numberValue(row[0])
    const itemId = numberValue(row[1])
    const name = row[2] ?? ''
    if (!id || !itemId || !name) continue
    books[String(id)] = name
    books[String(itemId)] = name
    books[String(id + 546)] = name
  }
  return books
}

function loadSources(): Record<string, ItemSource[]> {
  const sources: Record<string, ItemSource[]> = {}

  for (const row of readRows('GatheringItem.csv')) {
    const itemId = numberValue(row[1])
    addSource(sources, itemId, { kind: 'gathering' })
  }

  const gilShopNames = new Map<number, string>()
  for (const row of readRows('GilShop.csv')) {
    const id = numberValue(row[0])
    const name = row[1] ?? ''
    if (id) gilShopNames.set(id, name)
  }

  for (const row of readRows('GilShopItem.csv')) {
    const shopId = keyBase(row[0])
    const itemId = numberValue(row[1])
    const shopName = gilShopNames.get(shopId) || '金币商店'
    addSource(sources, itemId, { kind: 'gilShop', shopName })
  }

  for (const row of readRows('SpecialShop.csv')) {
    const shopName = row[1] || '兑换'
    const useCurrencyType = numberValue(row[2042])
    const costGroups = [
      { itemBase: 482, countBase: 542 },
      { itemBase: 722, countBase: 782 },
      { itemBase: 962, countBase: 1022 },
    ]

    for (let i = 0; i < 60; i += 1) {
      const receiveItem = numberValue(row[2 + i])
      if (!receiveItem) continue

      const costs: Array<{ itemId: number; count: number }> = []
      for (const group of costGroups) {
        const costItemId = numberValue(row[group.itemBase + i])
        const costCount = numberValue(row[group.countBase + i])
        if (costItemId && costCount) {
          costs.push({
            itemId: resolveSpecialShopCostItemId(useCurrencyType, costItemId),
            count: costCount,
          })
        }
      }

      if (costs.length) {
        addSource(sources, receiveItem, {
          kind: 'specialShop',
          shopName,
          costs,
        })
      }
    }
  }

  return sources
}

function tryGit(args: string[]): string | undefined {
  const result = Bun.spawnSync(['git', '-C', DATAMINING_DIR, ...args], {
    stdout: 'pipe',
    stderr: 'ignore',
  })
  if (result.exitCode !== 0) return undefined
  return new TextDecoder().decode(result.stdout).trim()
}

function versionInfo() {
  const commit = tryGit(['rev-parse', '--short=12', 'HEAD']) ?? 'local'
  const date = tryGit(['log', '-1', '--format=%cI']) ?? new Date().toISOString()
  return { commit, date }
}

function main() {
  console.log(`Reading ${DATAMINING_DIR}`)
  const items = loadItems()
  const recipes = loadRecipes()
  const recipeLevels = loadRecipeLevels()
  const secretRecipeBooks = loadSecretRecipeBooks()
  const sources = loadSources()
  const sourceCount = Object.values(sources).reduce((sum, list) => sum + list.length, 0)

  const data: CraftDataPackage = {
    generatedAt: new Date().toISOString(),
    source: DATAMINING_DIR,
    counts: {
      items: Object.keys(items).length,
      recipes: recipes.length,
      sources: sourceCount,
    },
    items,
    recipes,
    recipeLevels,
    secretRecipeBooks,
    sources,
  }

  if (!existsSync(OUT_DIR)) mkdirSync(OUT_DIR, { recursive: true })
  writeFileSync(join(OUT_DIR, 'craft-data.json'), JSON.stringify(data))
  writeFileSync(join(OUT_DIR, 'version.json'), JSON.stringify(versionInfo()))

  console.log(`Items: ${data.counts.items}`)
  console.log(`Recipes: ${data.counts.recipes}`)
  console.log(`Sources: ${data.counts.sources}`)
  console.log(`Output: ${join(OUT_DIR, 'craft-data.json')}`)
}

main()
