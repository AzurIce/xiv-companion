import type { JSX } from 'solid-js'
import { createEffect, createMemo, createResource, createSignal, For, Show } from 'solid-js'
import {
  ChevronDown,
  ChevronRight,
  CircleCheck,
  Coins,
  ExternalLink,
  Fish,
  Info,
  Leaf,
  PackageSearch,
  Search,
  Shuffle,
  X,
} from 'lucide-solid'
import {
  buildCraftTree,
  collapseKey,
  CRAFT_TYPE_ABBRS,
  CRAFT_TYPE_NAMES,
  craftableRecipes,
  defaultSourceIndex,
  formatInteger,
  createCraftDataEngine,
  getIconUrls,
  getItem,
  getItemName,
  loadCraftData,
  resolveSource,
  sourceLabel,
  summarizeMaterials,
  type CraftItem,
  type CraftDataPackage,
  type CraftRecipe,
  type CraftTreeNode,
  type ItemSource,
  type SourceChoice,
  cx,
} from '../../lib'
import { Badge, Button, EmptyState, Input } from '../../ui'

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

function recipeLevelLabel(data: CraftDataPackage, recipe: CraftRecipe) {
  if (recipe.secretRecipeBook > 0) {
    return data.secretRecipeBooks[String(recipe.secretRecipeBook)] ?? '秘籍'
  }
  return `Lv.${data.recipeLevels[String(recipe.recipeLevelTableId)]?.classJobLevel ?? 1}`
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

function costListLabel(data: CraftDataPackage, costs: Array<{ itemId: number; amount: number }>) {
  return costs
    .map((cost) => `${getItemName(data, cost.itemId)} x${formatInteger(cost.amount)}`)
    .join(' + ')
}

type SourceDisplayGroup = {
  key: string
  source: ItemSource
  indices: number[]
  costLabel?: string
  details: string[]
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

  return [...groups.values()]
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

const MARKET_WORLD_DC_REGION = '中国'
const UNIVERSALIS_BASE_URL = import.meta.env.DEV ? '/api/universalis' : 'https://universalis.app'

type MarketQuote = {
  itemId: number
  unitPrice: number
  basis: string
  lastUploadTime?: number
}

function isMarketable(item?: CraftItem) {
  return (item?.itemSearchCategory ?? 0) > 0
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
    lastUploadTime: raw.worldUploadTimes?.[0]?.timestamp,
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

type MaterialPlanEntry = {
  itemId: number
  amount: number
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

function exchangeGroupKey(source: Extract<ItemSource, { kind: 'specialShop' }>) {
  return `${source.shopName}|${source.costs.map((cost) => cost.itemId).join('+')}`
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
      <div
        class={cx(
          'grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm',
          props.onInspect && 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        )}
        role={props.onInspect ? 'button' : undefined}
        tabIndex={props.onInspect ? 0 : undefined}
        onKeyDown={(event) => {
          if (!props.onInspect || (event.key !== 'Enter' && event.key !== ' ')) return
          event.preventDefault()
          props.onInspect(props.entry)
        }}
      >
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

function ExchangeGroupPanel(props: {
  data: CraftDataPackage
  group: ExchangePlanGroup
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
  onInspect: (entry: MaterialPlanEntry) => void
  onInspectItem: (itemId: number, amount: number) => void
}) {
  return (
    <div class="overflow-hidden rounded-md border bg-[#fbf9ff]">
      <div class="grid grid-cols-1 lg:grid-cols-[150px_minmax(0,1fr)] xl:grid-cols-1 2xl:grid-cols-[160px_minmax(0,1fr)]">
        <div class="border-b bg-[#f0ebff] p-3 lg:border-b-0 lg:border-r xl:border-b xl:border-r-0 2xl:border-b-0 2xl:border-r">
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

function SummaryItemRow(props: {
  data: CraftDataPackage
  itemId: number
  amount: number
  class: string
  meta?: JSX.Element
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
        <Show when={props.meta}>
          <div class="whitespace-normal break-words text-xs leading-snug text-muted-foreground">{props.meta}</div>
        </Show>
      </div>
      <Badge variant="outline" class="bg-background/70">x{formatInteger(props.amount)}</Badge>
    </div>
  )
}

function NodeDetailDialog(props: {
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
                          <Show
                            when={group.details.length > 1}
                            fallback={group.details[0]}
                          >
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

function TreeNode(props: {
  data: CraftDataPackage
  node: CraftTreeNode
  depth: number
  collapsed: Set<string>
  sourceChoices: Map<number, SourceChoice>
  onToggle: (key: string) => void
  onSelect: (node: CraftTreeNode) => void
}) {
  const item = () => getItem(props.data, props.node.itemId)
  const key = () => collapseKey(props.node.itemId, props.depth)
  const isCollapsed = () => props.collapsed.has(key())
  const isCraftable = () => props.node.children.length > 0
  const recipe = () => props.node.recipe
  const countsAsLeaf = () => !isCraftable() || isCollapsed()
  const toneClass = () => countsAsLeaf()
    ? leafToneClass(props.data, props.node, props.sourceChoices)
    : 'border-l-transparent bg-background hover:bg-accent/70'

  return (
    <div>
      <div
        class={cx(
          'group relative cursor-pointer rounded-sm border-l-2 px-2 py-1 text-sm transition-colors',
          toneClass(),
        )}
        style={{ 'padding-left': `${8 + props.depth * 18}px` }}
        onClick={() => props.onSelect(props.node)}
      >
        <div class="grid grid-cols-[1.25rem_1.5rem_minmax(0,1fr)_auto] items-center gap-2">
          <button
            type="button"
            class="flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-background"
            aria-label={isCollapsed() ? '展开' : '折叠'}
            onClick={(event) => {
              event.stopPropagation()
              if (isCraftable()) props.onToggle(key())
            }}
          >
            <Show when={isCraftable()} fallback={<span class="h-1 w-1 rounded-full bg-muted-foreground/40" />}>
              <Show when={isCollapsed()} fallback={<ChevronDown class="h-4 w-4" />}>
                <ChevronRight class="h-4 w-4" />
              </Show>
            </Show>
          </button>

          <ItemIcon icon={item()?.icon ?? 0} size="sm" />

          <div class="min-w-0">
            <div class="truncate font-medium">{getItemName(props.data, props.node.itemId)}</div>
            <Show when={recipe()}>
              {(value) => (
                <div class="truncate text-xs text-muted-foreground">
                  {CRAFT_TYPE_ABBRS[Math.min(value().craftType, 7)]} · {recipeLevelLabel(props.data, value())}
                </div>
              )}
            </Show>
          </div>

          <div class="flex items-center gap-1">
            <Show when={countsAsLeaf()}>
              <Badge variant="outline" class="bg-background/70">叶</Badge>
            </Show>
            <Badge variant="outline" class="bg-background/70">x{formatInteger(props.node.amountNeeded)}</Badge>
          </div>
        </div>
      </div>

      <Show when={isCraftable() && !isCollapsed()}>
        <For each={props.node.children}>
          {(child) => (
            <TreeNode
              data={props.data}
              node={child}
              depth={props.depth + 1}
              collapsed={props.collapsed}
              sourceChoices={props.sourceChoices}
              onToggle={props.onToggle}
              onSelect={props.onSelect}
            />
          )}
        </For>
      </Show>
    </div>
  )
}

export default function CraftingPage() {
  const [craftData] = createResource(loadCraftData)
  const [craftEngine] = createResource(craftData, createCraftDataEngine)
  const [query, setQuery] = createSignal('')
  const [craftType, setCraftType] = createSignal<number | undefined>()
  const [selectedRecipeId, setSelectedRecipeId] = createSignal<number | undefined>()
  const [detailTarget, setDetailTarget] = createSignal<DetailTarget | undefined>()
  const [collapsed, setCollapsed] = createSignal(new Set<string>())
  const [sourceChoices, setSourceChoices] = createSignal(new Map<number, SourceChoice>())

  const recipes = createMemo(() => {
    const engine = craftEngine()
    if (!engine) return []
    return craftableRecipes(engine, craftType(), query(), 300)
  })

  const selectedRecipe = createMemo(() => {
    const id = selectedRecipeId()
    return recipes().find((recipe) => recipe.id === id) ?? recipes()[0]
  })

  createEffect(() => {
    const recipe = selectedRecipe()
    if (!recipe) {
      if (selectedRecipeId() != null) setSelectedRecipeId(undefined)
      if (detailTarget()) setDetailTarget(undefined)
      return
    }
    if (selectedRecipeId() !== recipe.id) {
      setSelectedRecipeId(recipe.id)
      setDetailTarget(undefined)
      setCollapsed(new Set<string>())
      setSourceChoices(new Map<number, SourceChoice>())
    }
  })

  const tree = createMemo(() => {
    const recipe = selectedRecipe()
    const engine = craftEngine()
    if (!recipe || !engine) return undefined
    return buildCraftTree(engine, recipe.resultItemId, 1)
  })

  const treeView = createMemo(() => {
    const data = craftData()
    const root = tree()
    if (!data || !root) return undefined
    return { data, root }
  })

  const detailView = createMemo(() => {
    const data = craftData()
    const target = detailTarget()
    if (!data || !target) return undefined
    return { data, target }
  })

  const materials = createMemo(() => {
    const root = tree()
    return root ? summarizeMaterials(root, collapsed()) : []
  })

  const materialPlan = createMemo(() => {
    const data = craftData()
    const empty = {
      gathering: [] as MaterialPlanEntry[],
      shops: [] as MaterialPlanEntry[],
      market: [] as MaterialPlanEntry[],
      exchangeGroups: [] as ExchangePlanGroup[],
      owned: [] as MaterialPlanEntry[],
      unknown: [] as MaterialPlanEntry[],
      gilTotal: 0,
    }
    if (!data) return empty

    const gathering: typeof empty.gathering = []
    const shops: typeof empty.shops = []
    const market: typeof empty.market = []
    const owned: typeof empty.owned = []
    const unknown: typeof empty.unknown = []
    const exchangeGroups = new Map<string, ExchangePlanGroup & { costMap: Map<number, number> }>()
    let gilTotal = 0
    const choices = sourceChoices()

    for (const material of materials()) {
      const sources = data.sources[String(material.itemId)] ?? []
      const item = getItem(data, material.itemId)
      const marketable = isMarketable(item)
      const choice = choices.get(material.itemId)
      const source = resolveSource(material.itemId, sources, choices)
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
        const gil = (getItem(data, material.itemId)?.priceMid ?? 0) * material.amount
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
    let priced = 0
    let missing = 0
    for (const entry of materialPlan().market) {
      const unitPrice = quotes?.get(entry.itemId)?.unitPrice
      if (unitPrice) {
        total += unitPrice * entry.amount
        priced += 1
      } else {
        missing += 1
      }
    }
    return { total, priced, missing }
  })

  const selectRecipe = (recipe: CraftRecipe) => {
    setSelectedRecipeId(recipe.id)
    setDetailTarget(undefined)
    setCollapsed(new Set<string>())
    setSourceChoices(new Map<number, SourceChoice>())
  }

  const toggleCollapsed = (key: string) => {
    setCollapsed((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  const chooseSource = (itemId: number, choice: SourceChoice | undefined) => {
    setSourceChoices((current) => {
      const next = new Map(current)
      if (!choice) next.delete(itemId)
      else next.set(itemId, choice)
      return next
    })
  }

  const inspectNode = (node: CraftTreeNode) => {
    setDetailTarget({
      itemId: node.itemId,
      amountNeeded: node.amountNeeded,
      recipe: node.recipe,
    })
  }

  const inspectPlanEntry = (entry: MaterialPlanEntry) => {
    setDetailTarget({ itemId: entry.itemId, amountNeeded: entry.amount })
  }

  const inspectItem = (itemId: number, amountNeeded: number) => {
    setDetailTarget({ itemId, amountNeeded })
  }

  const detailRecipe = createMemo(() => {
    const target = detailTarget()
    const engine = craftEngine()
    if (!target) return undefined
    if (target.recipe) return target.recipe
    return engine ? buildCraftTree(engine, target.itemId, target.amountNeeded).recipe : undefined
  })

  const marketMeta = (entry: MaterialPlanEntry) => {
    if (marketQuotes.loading) return `估价载入中 · ${MARKET_WORLD_DC_REGION}`
    if (marketQuotes.error) return '估价失败'
    const quote = marketQuotes()?.get(entry.itemId)
    if (!quote) return '暂无市场价格'
    return `${formatInteger(quote.unitPrice)}G / 个 · ${formatInteger(quote.unitPrice * entry.amount)}G · ${quote.basis}`
  }

  return (
    <div class="flex min-h-screen flex-col lg:h-screen lg:min-h-0 lg:overflow-hidden">
      <div class="shrink-0 border-b bg-background px-4 py-4 sm:px-6 lg:px-8">
        <div class="mx-auto flex max-w-[1600px] flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <div class="text-sm text-muted-foreground">工具 / 合成检索</div>
            <h1 class="text-2xl font-semibold">合成检索</h1>
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

      <div class="grid w-full flex-1 lg:min-h-0 lg:grid-cols-[320px_minmax(0,1fr)] lg:grid-rows-[minmax(0,1fr)_320px] xl:grid-cols-[340px_minmax(0,1fr)_380px] xl:grid-rows-1 2xl:grid-cols-[360px_minmax(0,1fr)_420px]">
        <aside class="flex h-[340px] flex-col overflow-hidden border-b bg-card sm:h-[380px] lg:row-span-2 lg:h-auto lg:min-h-0 lg:border-b-0 lg:border-r xl:row-span-1">
          <div class="border-b p-3">
            <div class="relative">
              <Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query()}
                onInput={(event) => setQuery(event.currentTarget.value)}
                placeholder="搜索物品或 ID"
                class="pl-9 pr-9"
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

            <div class="mt-3 flex flex-wrap gap-1.5">
              <Button
                size="sm"
                variant={craftType() == null ? 'primary' : 'outline'}
                onClick={() => setCraftType(undefined)}
              >
                全部
              </Button>
              <For each={CRAFT_TYPE_ABBRS}>
                {(label, i) => (
                  <Button
                    size="sm"
                    variant={craftType() === i() ? 'primary' : 'outline'}
                    onClick={() => setCraftType(i())}
                  >
                    {label}
                  </Button>
                )}
              </For>
            </div>
          </div>

          <div class="min-h-0 flex-1 overflow-y-auto p-2">
            <Show
              when={craftData()}
              fallback={
                <div class="space-y-2 p-2">
                  <For each={Array.from({ length: 12 })}>
                    {() => <div class="h-12 rounded-md bg-muted" />}
                  </For>
                </div>
              }
            >
              {(data) => (
                <Show
                  when={recipes().length > 0}
                  fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="没有匹配的配方" />}
                >
                  <For each={recipes()}>
                    {(recipe) => {
                      const item = () => getItem(data(), recipe.resultItemId)
                      const active = () => selectedRecipe()?.id === recipe.id
                      return (
                        <button
                          type="button"
                          class={[
                            'mb-1 grid w-full grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors',
                            active() ? 'bg-accent text-foreground' : 'hover:bg-accent/70',
                          ].join(' ')}
                          onClick={() => selectRecipe(recipe)}
                        >
                          <ItemIcon icon={item()?.icon ?? 0} />
                          <div class="min-w-0">
                            <div class="truncate font-medium">{getItemName(data(), recipe.resultItemId)}</div>
                            <div class="truncate text-xs text-muted-foreground">
                              {CRAFT_TYPE_NAMES[Math.min(recipe.craftType, 7)]} · {recipeLevelLabel(data(), recipe)}
                            </div>
                          </div>
                          <Badge variant="outline">#{recipe.resultItemId}</Badge>
                        </button>
                      )
                    }}
                  </For>
                </Show>
              )}
            </Show>
          </div>
        </aside>

        <section class="min-h-[520px] overflow-hidden bg-background lg:min-h-0">
          <Show
            when={treeView()}
            fallback={
              <EmptyState
                icon={<PackageSearch class="h-6 w-6" />}
                title="合成数据未载入"
                description="运行 update-craft-data 后刷新页面"
              />
            }
          >
            {(view) => {
              const data = () => view().data
              const root = () => view().root
              const recipe = () => selectedRecipe()
              const item = () => recipe() ? getItem(data(), recipe()!.resultItemId) : undefined

              return (
                <div class="flex h-full min-h-[520px] flex-col lg:min-h-0">
                  <div class="flex items-center gap-3 border-b p-4">
                    <ItemIcon icon={item()?.icon ?? 0} />
                    <div class="min-w-0 flex-1">
                      <div class="truncate text-base font-semibold">{recipe() ? getItemName(data(), recipe()!.resultItemId) : ''}</div>
                      <div class="text-sm text-muted-foreground">
                        {recipe() ? `${CRAFT_TYPE_NAMES[Math.min(recipe()!.craftType, 7)]} · ${recipeLevelLabel(data(), recipe()!)}` : ''}
                      </div>
                    </div>
                    <Badge variant="secondary">x{recipe()?.resultAmount ?? 1}</Badge>
                  </div>

                  <div class="min-h-0 flex-1 overflow-y-auto p-3">
                    <TreeNode
                      data={data()}
                      node={root()}
                      depth={0}
                      collapsed={collapsed()}
                      sourceChoices={sourceChoices()}
                      onToggle={toggleCollapsed}
                      onSelect={inspectNode}
                    />
                  </div>
                </div>
              )
            }}
          </Show>
        </section>

        <aside class="flex min-h-[420px] flex-col overflow-hidden border-t bg-card lg:col-start-2 lg:row-start-2 lg:min-h-0 xl:col-start-auto xl:row-start-auto xl:border-l xl:border-t-0">
          <section class="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div class="shrink-0 border-b p-4">
              <div class="flex items-start gap-2">
                <Info class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
                <div class="min-w-0">
                  <div class="text-base font-semibold">准备计划</div>
                  <div class="mt-1 text-xs text-muted-foreground">
                    由合成树叶子节点的当前来源生成；折叠中间节点时，该节点会作为叶子材料计入。
                  </div>
                </div>
              </div>
              <div class="mt-3 flex flex-wrap gap-2">
                <Badge variant="outline">叶子 {materials().length}</Badge>
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
                <Show when={marketCost().total > 0}>
                  <Badge variant="outline">估 {formatInteger(marketCost().total)}G</Badge>
                </Show>
                <Show when={materialPlan().owned.length > 0}>
                  <Badge variant="outline"><CircleCheck class="mr-1 h-3 w-3" />已拥有 {materialPlan().owned.length}</Badge>
                </Show>
              </div>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-3">
              <Show when={craftData()}>
                {(data) => (
                  <div class="space-y-4">
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
                                data={data()}
                                group={group}
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
                                onInspectItem={inspectItem}
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
                                data={data()}
                                entry={entry}
                                class="border-l-emerald-200 bg-emerald-50/80"
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
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
                                data={data()}
                                entry={entry}
                                class="border-l-amber-200 bg-amber-50/80"
                                meta={`${entry.shopName}${entry.gil ? ` · ${formatInteger(entry.gil)}G` : ''}`}
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
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
                            <Show when={marketQuotes.loading} fallback={marketCost().total > 0 ? `估 ${formatInteger(marketCost().total)}G` : `区域 ${MARKET_WORLD_DC_REGION}`}>
                              估价载入中
                            </Show>
                          </div>
                        </div>
                        <div class="space-y-2">
                          <For each={materialPlan().market}>
                            {(entry) => (
                              <MaterialPlanRow
                                data={data()}
                                entry={entry}
                                class="border-l-[#93c5fd] bg-[#eff6ff]"
                                meta={marketMeta(entry)}
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
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
                                data={data()}
                                entry={entry}
                                class="border-l-border bg-background"
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
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
                                data={data()}
                                entry={entry}
                                class="border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground"
                                onChoose={chooseSource}
                                onInspect={inspectPlanEntry}
                              />
                            )}
                          </For>
                        </div>
                      </section>
                    </Show>

                    <Show
                      when={
                        materialPlan().exchangeGroups.length === 0
                        && materialPlan().gathering.length === 0
                        && materialPlan().shops.length === 0
                        && materialPlan().market.length === 0
                        && materialPlan().unknown.length === 0
                        && materialPlan().owned.length === 0
                      }
                    >
                      <EmptyState
                        icon={<PackageSearch class="h-6 w-6" />}
                        title="暂无材料"
                        description="选择一个配方后会在这里生成叶子材料清单"
                      />
                    </Show>
                  </div>
                )}
              </Show>
            </div>
          </section>

        </aside>
      </div>

      <Show when={detailView()}>
        {(view) => (
          <NodeDetailDialog
            data={view().data}
            target={view().target}
            recipe={detailRecipe()}
            onClose={() => setDetailTarget(undefined)}
          />
        )}
      </Show>
    </div>
  )
}
