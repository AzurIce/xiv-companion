import type { JSX } from 'solid-js'
import { createEffect, createMemo, createResource, createSignal, For, Show } from 'solid-js'
import {
  ChevronDown,
  ChevronRight,
  CircleCheck,
  Coins,
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
  createCraftDataIndex,
  CRAFT_TYPE_ABBRS,
  CRAFT_TYPE_NAMES,
  defaultSourceIndex,
  formatInteger,
  getIconUrls,
  getItem,
  getItemName,
  loadCraftData,
  resolveSource,
  sourceLabel,
  summarizeMaterials,
  type CraftDataPackage,
  type CraftRecipe,
  type CraftTreeNode,
  type ItemSource,
  type SourceChoice,
  cx,
} from '@xiv-companian/shared'
import { Badge, Button, EmptyState, Input, Separator } from '@xiv-companian/ui'

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
  if (source.kind === 'gilShop') return <Coins class="h-3.5 w-3.5" />
  if (source.kind === 'specialShop') return <Shuffle class="h-3.5 w-3.5" />
  return <Leaf class="h-3.5 w-3.5" />
}

function sourceDetail(data: CraftDataPackage, source: ItemSource, amount: number) {
  if (source.kind === 'gilShop') return source.shopName
  if (source.kind === 'specialShop') {
    return `${source.shopName} · ${sourceCostLabel(data, source, amount)}`
  }
  return '采矿 / 园艺'
}

function sourceAbbr(source: ItemSource) {
  if (source.kind === 'gilShop') return '店'
  if (source.kind === 'specialShop') return '换'
  return '采'
}

function sourceCostLabel(data: CraftDataPackage, source: ItemSource, amount: number) {
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

function sourceToneClass(source: ItemSource | undefined, ignored = false) {
  if (ignored) return 'border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground'
  if (source?.kind === 'gathering') return 'border-l-emerald-200 bg-emerald-50/80'
  if (source?.kind === 'specialShop') return 'border-l-[#d7c7ff] bg-[#f5efff]'
  if (source?.kind === 'gilShop') return 'border-l-amber-200 bg-amber-50/80'
  return 'border-l-border bg-background'
}

function sourceButtonClass(source: ItemSource | undefined, active: boolean, ignored = false) {
  if (ignored && active) return 'border-[#a8a29e] bg-[#e7e5e4] text-[#44403c]'
  if (!active) return 'border-border bg-background/80 text-muted-foreground hover:bg-background hover:text-foreground'
  if (source?.kind === 'gathering') return 'border-emerald-200 bg-[#dff5e5] text-[#166534]'
  if (source?.kind === 'specialShop') return 'border-[#bfa7ff] bg-[#e4d9ff] text-[#3b2778]'
  if (source?.kind === 'gilShop') return 'border-amber-200 bg-[#fff0bf] text-[#854d0e]'
  return 'border-border bg-secondary text-secondary-foreground'
}

function SourceChoiceControls(props: {
  data: CraftDataPackage
  itemId: number
  amount: number
  sources: ItemSource[]
  choice?: SourceChoice
  onChoose: (itemId: number, choice: SourceChoice | undefined) => void
}) {
  const ignored = () => props.choice?.kind === 'ignore'
  const currentSourceIndex = () => {
    if (props.choice?.kind === 'index') return props.choice.index
    if (props.choice?.kind === 'ignore') return undefined
    return defaultSourceIndex(props.sources)
  }

  return (
    <div class="flex flex-wrap gap-1" onClick={(event) => event.stopPropagation()}>
      <button
        type="button"
        class={cx(
          'inline-flex h-6 items-center gap-1 rounded border px-2 text-[11px] font-medium transition-colors',
          sourceButtonClass(undefined, ignored(), true),
        )}
        onClick={() => props.onChoose(props.itemId, ignored() ? undefined : { kind: 'ignore' })}
        title="已持有"
        aria-label="已持有"
      >
        <CircleCheck class="h-3 w-3" />
        已持有
      </button>

      <Show
        when={props.sources.length > 0}
        fallback={<span class="inline-flex h-6 items-center rounded border bg-background/80 px-2 text-[11px] text-muted-foreground">无来源</span>}
      >
        <For each={props.sources}>
          {(source, i) => {
            const active = () => !ignored() && currentSourceIndex() === i()
            return (
              <button
                type="button"
                class={cx(
                  'inline-flex h-6 max-w-full items-center gap-1 rounded border px-2 text-[11px] font-medium transition-colors',
                  sourceButtonClass(source, active()),
                )}
                onClick={() => props.onChoose(props.itemId, { kind: 'index', index: i() })}
                title={sourceDetail(props.data, source, props.amount)}
              >
                {sourceIcon(source)}
                <span>{sourceLabel(source)}</span>
                <Show when={sourceCostLabel(props.data, source, props.amount)}>
                  {(label) => (
                    <span class={cx('max-w-32 truncate', active() ? 'opacity-90' : 'opacity-70')}>
                      {label()}
                    </span>
                  )}
                </Show>
              </button>
            )
          }}
        </For>
      </Show>
    </div>
  )
}

function SummaryItemRow(props: {
  data: CraftDataPackage
  itemId: number
  amount: number
  class: string
  meta?: JSX.Element
}) {
  const item = () => getItem(props.data, props.itemId)

  return (
    <div class={cx('mb-1 grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-sm border-l-2 px-2 py-1.5 text-sm', props.class)}>
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

function TreeNode(props: {
  data: CraftDataPackage
  node: CraftTreeNode
  depth: number
  collapsed: Set<string>
  sourceChoices: Map<number, SourceChoice>
  selectedItemId?: number
  onToggle: (key: string) => void
  onSelect: (node: CraftTreeNode) => void
  onChooseSource: (itemId: number, choice: SourceChoice | undefined) => void
}) {
  const item = () => getItem(props.data, props.node.itemId)
  const key = () => collapseKey(props.node.itemId, props.depth)
  const isCollapsed = () => props.collapsed.has(key())
  const isCraftable = () => props.node.children.length > 0
  const isSelected = () => props.selectedItemId === props.node.itemId
  const recipe = () => props.node.recipe
  const countsAsLeaf = () => !isCraftable() || isCollapsed()
  const sources = () => props.data.sources[String(props.node.itemId)] ?? []
  const choices = () => props.sourceChoices ?? new Map<number, SourceChoice>()
  const ignored = () => choices().get(props.node.itemId)?.kind === 'ignore'
  const currentSource = () => countsAsLeaf() && !ignored()
    ? resolveSource(props.node.itemId, sources(), choices())
    : undefined

  return (
    <div>
      <div
        class={cx(
          'group cursor-pointer rounded-sm border-l-2 px-2 py-1 text-sm transition-colors',
          isSelected()
            ? 'border-l-[#60a5fa] bg-[#dceeff] text-[#123047] ring-1 ring-[#a9d3ff]'
            : sourceToneClass(currentSource(), ignored()),
          !isSelected() && !countsAsLeaf() && 'hover:bg-accent/70',
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
              <Show
                when={ignored()}
                fallback={
                  <Show when={currentSource()}>
                    {(source) => <Badge variant="outline" class="bg-background/70">{sourceAbbr(source())}</Badge>}
                  </Show>
                }
              >
                <Badge variant="outline" class="bg-background/70">持</Badge>
              </Show>
            </Show>
            <Badge variant="outline" class="bg-background/70">x{formatInteger(props.node.amountNeeded)}</Badge>
          </div>
        </div>

        <Show when={countsAsLeaf()}>
          <div class="mt-1 pl-11">
            <SourceChoiceControls
              data={props.data}
              itemId={props.node.itemId}
              amount={props.node.amountNeeded}
              sources={sources()}
              choice={choices().get(props.node.itemId)}
              onChoose={props.onChooseSource}
            />
          </div>
        </Show>
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
              selectedItemId={props.selectedItemId}
              onToggle={props.onToggle}
              onSelect={props.onSelect}
              onChooseSource={props.onChooseSource}
            />
          )}
        </For>
      </Show>
    </div>
  )
}

export default function CraftingPage() {
  const [craftData] = createResource(loadCraftData)
  const [query, setQuery] = createSignal('')
  const [craftType, setCraftType] = createSignal<number | undefined>()
  const [selectedRecipeId, setSelectedRecipeId] = createSignal<number | undefined>()
  const [selectedNode, setSelectedNode] = createSignal<CraftTreeNode | undefined>()
  const [collapsed, setCollapsed] = createSignal(new Set<string>())
  const [sourceChoices, setSourceChoices] = createSignal(new Map<number, SourceChoice>())

  const index = createMemo(() => {
    const data = craftData()
    return data ? createCraftDataIndex(data) : undefined
  })

  const recipes = createMemo(() => {
    const idx = index()
    const data = craftData()
    if (!idx || !data) return []

    const source = craftType() == null
      ? Array.from(idx.craftableByType.values()).flat()
      : idx.craftableByType.get(craftType()!) ?? []
    const text = query().trim().toLowerCase()

    return source
      .filter((recipe) => {
        if (!text) return true
        const name = getItemName(data, recipe.resultItemId).toLowerCase()
        return name.includes(text) || String(recipe.resultItemId).includes(text) || String(recipe.id).includes(text)
      })
      .slice(0, 300)
  })

  const selectedRecipe = createMemo(() => {
    const id = selectedRecipeId()
    return recipes().find((recipe) => recipe.id === id) ?? recipes()[0]
  })

  createEffect(() => {
    const recipe = selectedRecipe()
    if (!recipe) {
      if (selectedRecipeId() != null) setSelectedRecipeId(undefined)
      if (selectedNode()) setSelectedNode(undefined)
      return
    }
    if (selectedRecipeId() !== recipe.id) {
      setSelectedRecipeId(recipe.id)
      setSelectedNode(undefined)
      setCollapsed(new Set<string>())
      setSourceChoices(new Map<number, SourceChoice>())
    }
  })

  const tree = createMemo(() => {
    const recipe = selectedRecipe()
    const idx = index()
    if (!recipe || !idx) return undefined
    return buildCraftTree(recipe.resultItemId, 1, idx)
  })

  const treeView = createMemo(() => {
    const data = craftData()
    const root = tree()
    if (!data || !root) return undefined
    return { data, root }
  })

  const detailView = createMemo(() => {
    const data = craftData()
    const node = selectedNode()
    if (!data || !node) return undefined
    return { data, node }
  })

  createEffect(() => {
    const current = tree()
    if (current && !selectedNode()) {
      setSelectedNode(current)
    }
  })

  const materials = createMemo(() => {
    const root = tree()
    return root ? summarizeMaterials(root, collapsed()) : []
  })

  const materialPlan = createMemo(() => {
    const data = craftData()
    const empty = {
      gathering: [] as Array<{ itemId: number; amount: number }>,
      shops: [] as Array<{ itemId: number; amount: number; shopName: string; gil: number }>,
      exchanges: [] as Array<{ itemId: number; amount: number; shopName: string; costs: Array<{ itemId: number; amount: number }> }>,
      exchangeCosts: [] as Array<{ itemId: number; amount: number }>,
      owned: [] as Array<{ itemId: number; amount: number }>,
      unknown: [] as Array<{ itemId: number; amount: number }>,
      gilTotal: 0,
    }
    if (!data) return empty

    const gathering: typeof empty.gathering = []
    const shops: typeof empty.shops = []
    const exchanges: typeof empty.exchanges = []
    const owned: typeof empty.owned = []
    const unknown: typeof empty.unknown = []
    const exchangeCosts = new Map<number, number>()
    let gilTotal = 0
    const choices = sourceChoices()

    for (const material of materials()) {
      const sources = data.sources[String(material.itemId)] ?? []
      const choice = choices.get(material.itemId)
      if (choice?.kind === 'ignore') {
        owned.push(material)
        continue
      }
      const source = resolveSource(material.itemId, sources, choices)
      if (source?.kind === 'gilShop') {
        const gil = (getItem(data, material.itemId)?.priceMid ?? 0) * material.amount
        gilTotal += gil
        shops.push({ ...material, shopName: source.shopName, gil })
      } else if (source?.kind === 'specialShop') {
        const costs = source.costs.map((cost) => ({
          itemId: cost.itemId,
          amount: cost.count * material.amount,
        }))
        exchanges.push({
          ...material,
          shopName: source.shopName,
          costs,
        })
        for (const cost of costs) {
          exchangeCosts.set(cost.itemId, (exchangeCosts.get(cost.itemId) ?? 0) + cost.amount)
        }
      } else if (source?.kind === 'gathering') {
        gathering.push(material)
      } else {
        unknown.push(material)
      }
    }

    return {
      gathering,
      shops,
      exchanges,
      exchangeCosts: [...exchangeCosts.entries()]
        .map(([itemId, amount]) => ({ itemId, amount }))
        .sort((a, b) => a.itemId - b.itemId),
      owned,
      unknown,
      gilTotal,
    }
  })

  const selectRecipe = (recipe: CraftRecipe) => {
    setSelectedRecipeId(recipe.id)
    setSelectedNode(undefined)
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

      <div class="grid w-full flex-1 lg:min-h-0 lg:grid-cols-[320px_minmax(0,1fr)] lg:grid-rows-[minmax(0,1fr)_320px] xl:grid-cols-[340px_minmax(0,1fr)_360px] xl:grid-rows-1">
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
                      selectedItemId={selectedNode()?.itemId}
                      onToggle={toggleCollapsed}
                      onSelect={setSelectedNode}
                      onChooseSource={chooseSource}
                    />
                  </div>
                </div>
              )
            }}
          </Show>
        </section>

        <aside class="grid min-h-[520px] overflow-hidden border-t bg-card lg:col-start-2 lg:row-start-2 lg:min-h-0 lg:grid-cols-[minmax(0,1fr)_320px] lg:grid-rows-1 xl:col-start-auto xl:row-start-auto xl:min-h-0 xl:grid-cols-1 xl:grid-rows-[minmax(0,1fr)_auto] xl:border-l xl:border-t-0">
          <section class="flex min-h-[360px] flex-col overflow-hidden lg:min-h-0">
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
                <Show when={materialPlan().exchangeCosts.length > 0}>
                  <Badge variant="outline"><Shuffle class="mr-1 h-3 w-3" />兑换成本 {materialPlan().exchangeCosts.length}</Badge>
                </Show>
                <Show when={materialPlan().gilTotal > 0}>
                  <Badge variant="warning"><Coins class="mr-1 h-3 w-3" />{formatInteger(materialPlan().gilTotal)}G</Badge>
                </Show>
                <Show when={materialPlan().owned.length > 0}>
                  <Badge variant="outline"><CircleCheck class="mr-1 h-3 w-3" />已持有 {materialPlan().owned.length}</Badge>
                </Show>
              </div>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-3">
              <Show when={craftData()}>
                {(data) => (
                  <div class="space-y-4">
                    <Show when={materialPlan().gathering.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                          <Leaf class="h-4 w-4 text-emerald-700" />
                          采集清单
                        </div>
                        <For each={materialPlan().gathering}>
                          {(material) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={material.itemId}
                              amount={material.amount}
                              class="border-l-emerald-200 bg-emerald-50/80"
                            />
                          )}
                        </For>
                      </section>
                    </Show>

                    <Show when={materialPlan().exchangeCosts.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                          <Shuffle class="h-4 w-4 text-[#3b2778]" />
                          兑换成本
                        </div>
                        <For each={materialPlan().exchangeCosts}>
                          {(cost) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={cost.itemId}
                              amount={cost.amount}
                              class="border-l-[#d7c7ff] bg-[#f5efff]"
                            />
                          )}
                        </For>
                      </section>
                    </Show>

                    <Show when={materialPlan().exchanges.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                          <Shuffle class="h-4 w-4 text-muted-foreground" />
                          兑换获得
                        </div>
                        <For each={materialPlan().exchanges}>
                          {(exchange) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={exchange.itemId}
                              amount={exchange.amount}
                              class="border-l-[#d7c7ff] bg-[#f5efff]"
                              meta={`${exchange.shopName} · ${costListLabel(data(), exchange.costs)}`}
                            />
                          )}
                        </For>
                      </section>
                    </Show>

                    <Show when={materialPlan().shops.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                          <Coins class="h-4 w-4 text-amber-700" />
                          商店购买
                        </div>
                        <For each={materialPlan().shops}>
                          {(shop) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={shop.itemId}
                              amount={shop.amount}
                              class="border-l-amber-200 bg-amber-50/80"
                              meta={`${shop.shopName}${shop.gil > 0 ? ` · ${formatInteger(shop.gil)}G` : ''}`}
                            />
                          )}
                        </For>
                      </section>
                    </Show>

                    <Show when={materialPlan().unknown.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold">
                          <PackageSearch class="h-4 w-4 text-muted-foreground" />
                          未解析来源
                        </div>
                        <For each={materialPlan().unknown}>
                          {(material) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={material.itemId}
                              amount={material.amount}
                              class="border-l-border bg-background"
                            />
                          )}
                        </For>
                      </section>
                    </Show>

                    <Show when={materialPlan().owned.length > 0}>
                      <section>
                        <div class="mb-2 flex items-center gap-2 text-sm font-semibold text-muted-foreground">
                          <CircleCheck class="h-4 w-4" />
                          已持有
                        </div>
                        <For each={materialPlan().owned}>
                          {(material) => (
                            <SummaryItemRow
                              data={data()}
                              itemId={material.itemId}
                              amount={material.amount}
                              class="border-l-[#a8a29e] bg-[#f1f0ee] text-muted-foreground"
                            />
                          )}
                        </For>
                      </section>
                    </Show>
                  </div>
                )}
              </Show>
            </div>
          </section>

          <section class="overflow-y-auto border-t p-4 lg:border-l lg:border-t-0 xl:border-l-0 xl:border-t">
            <div class="text-base font-semibold">节点详情</div>
            <Separator class="my-3" />
            <Show
              when={detailView()}
              fallback={<div class="text-sm text-muted-foreground">选择合成树或素材节点</div>}
            >
              {(view) => {
                const data = () => view().data
                const node = () => view().node
                const item = () => getItem(data(), node().itemId)
                const recipe = () => node().recipe ?? index()?.recipesByResult.get(node().itemId)?.[0]

                return (
                  <div class="space-y-4">
                    <div class="flex items-center gap-3">
                      <ItemIcon icon={item()?.icon ?? 0} />
                      <div class="min-w-0">
                        <div class="truncate text-sm font-semibold">{getItemName(data(), node().itemId)}</div>
                        <div class="text-xs text-muted-foreground">#{node().itemId}</div>
                      </div>
                    </div>

                    <div class="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                      <div class="text-muted-foreground">需求</div>
                      <div class="text-right font-medium">x{formatInteger(node().amountNeeded)}</div>
                      <Show when={item()?.priceLow}>
                        <div class="text-muted-foreground">收购</div>
                        <div class="text-right font-medium">{formatInteger(item()!.priceLow)}G</div>
                      </Show>
                      <Show when={recipe()}>
                        {(value) => (
                          <>
                            <div class="text-muted-foreground">职业</div>
                            <div class="text-right font-medium">{CRAFT_TYPE_NAMES[Math.min(value().craftType, 7)]}</div>
                            <div class="text-muted-foreground">等级</div>
                            <div class="text-right font-medium">{recipeLevelLabel(data(), value())}</div>
                            <div class="text-muted-foreground">产出</div>
                            <div class="text-right font-medium">x{value().resultAmount}</div>
                          </>
                        )}
                      </Show>
                    </div>

                    <Show when={(data().sources[String(node().itemId)] ?? []).length > 0}>
                      <div class="space-y-2">
                        <div class="text-sm font-medium">获取来源</div>
                        <For each={data().sources[String(node().itemId)] ?? []}>
                          {(source) => (
                            <div class={cx('flex items-start gap-2 rounded-sm border-l-2 p-2 text-sm', sourceToneClass(source))}>
                              <div class="mt-0.5 text-muted-foreground">{sourceIcon(source)}</div>
                              <div class="min-w-0 flex-1">
                                <div class="font-medium">{sourceLabel(source)}</div>
                                <div class="whitespace-normal break-words text-xs leading-snug text-muted-foreground">
                                  {sourceDetail(data(), source, node().amountNeeded)}
                                </div>
                              </div>
                            </div>
                          )}
                        </For>
                      </div>
                    </Show>
                  </div>
                )
              }}
            </Show>
          </section>
        </aside>
      </div>
    </div>
  )
}
