import { createEffect, createMemo, createResource, createSignal, For, Show } from 'solid-js'
import {
  ArrowLeft,
  BriefcaseBusiness,
  CopyPlus,
  Hammer,
  PackageSearch,
  Plus,
  RotateCcw,
  Trash2,
  X,
} from 'lucide-solid'
import {
  CRAFTER_GEAR_SLOTS,
  CRAFTER_JOBS,
  CRAFT_TYPE_ABBRS,
  createCharacterGearset,
  createDefaultCharacterState,
  equipmentDataSlotId,
  gearsetAttributes,
  gearsetMainHandCraftType,
  jobCanUseManipulation,
  loadCraftData,
  loadCharacterState,
  MANIPULATION_UNLOCK_LEVEL,
  MATERIA_STATS,
  MAX_CRAFTER_LEVEL,
  saveCharacterState,
  type CharacterMateria,
  type CharacterEquipmentPiece,
  type CharacterGearset,
  type CharacterState,
  type CraftDataPackage,
  type CrafterEquipmentItem,
  type CrafterJobId,
  type GearAttributes,
  type GearSlotId,
  type MateriaStat,
  cx,
  formatInteger,
} from '../lib'
import { Button, EmptyState, Input } from '../ui'

type CharacterTab = 'jobs' | 'gearsets'

type ActiveEquipmentPicker = {
  gearsetId: string
  slotId: GearSlotId
}

function id(prefix: string) {
  const value = globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
  return `${prefix}-${value}`
}

function clampNumber(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, Math.round(value)))
}

function craftTypeLabel(craftType: number | undefined) {
  return craftType == null ? '通用' : CRAFT_TYPE_ABBRS[craftType] ?? `职业 ${craftType}`
}

function equipmentPieceFromItem(item: CrafterEquipmentItem, slotId: GearSlotId): CharacterEquipmentPiece {
  return {
    id: id('equipment'),
    itemId: item.itemId,
    slotId,
    name: item.name,
    itemLevel: item.itemLevel,
    equipLevel: item.equipLevel,
    craftType: item.craftType,
    craftsmanship: item.craftsmanship,
    control: item.control,
    craftPoints: item.craftPoints,
    materiaSlotCount: item.materiaSlotCount,
    advancedMelding: item.advancedMelding,
    materia: [],
  }
}

function numericValue(value: string, fallback = 0) {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function updateState(setState: (fn: (current: CharacterState) => CharacterState) => void, fn: (state: CharacterState) => CharacterState) {
  setState((current) => fn({
    ...current,
    jobs: { ...current.jobs },
    gearsets: current.gearsets.map((gearset) => ({
      ...gearset,
      manualAttributes: { ...gearset.manualAttributes },
      baseAttributes: { ...gearset.baseAttributes },
      slots: cloneSlots(gearset.slots),
    })),
  }))
}

function cloneSlots(slots: Partial<Record<GearSlotId, CharacterEquipmentPiece>>) {
  return Object.fromEntries(
    Object.entries(slots).map(([slotId, piece]) => [
      slotId,
      piece ? { ...piece, materia: piece.materia.map((materia) => ({ ...materia })) } : piece,
    ]),
  ) as Partial<Record<GearSlotId, CharacterEquipmentPiece>>
}

function RangeField(props: {
  label: string
  value: number
  min: number
  max: number
  disabled?: boolean
  onInput: (value: number) => void
}) {
  const safeValue = () => clampNumber(props.value, props.min, props.max)

  return (
    <label class={cx('min-w-0 flex-1', props.disabled && 'opacity-50')}>
      <span class="sr-only">{props.label}</span>
      <input
        type="range"
        value={safeValue()}
        min={props.min}
        max={props.max}
        disabled={props.disabled}
        onInput={(event) => props.onInput(Number(event.currentTarget.value))}
        class="h-2 w-full accent-foreground disabled:opacity-50"
      />
    </label>
  )
}

function SegmentedTabs(props: { value: CharacterTab; onChange: (value: CharacterTab) => void }) {
  return (
    <div class="grid w-full grid-cols-2 gap-1 rounded-md bg-muted p-1 sm:w-auto sm:min-w-72">
      <button
        type="button"
        class={cx(
          'flex h-9 items-center justify-center gap-2 rounded text-sm font-medium transition-colors',
          props.value === 'jobs' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground',
        )}
        onClick={() => props.onChange('jobs')}
      >
        <Hammer class="h-4 w-4" />
        职业
      </button>
      <button
        type="button"
        class={cx(
          'flex h-9 items-center justify-center gap-2 rounded text-sm font-medium transition-colors',
          props.value === 'gearsets' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground',
        )}
        onClick={() => props.onChange('gearsets')}
      >
        <BriefcaseBusiness class="h-4 w-4" />
        配装
      </button>
    </div>
  )
}

function JobRow(props: {
  jobId: CrafterJobId
  state: CharacterState
  onLevelChange: (jobId: CrafterJobId, level: number) => void
  onManipulationChange: (jobId: CrafterJobId, enabled: boolean) => void
}) {
  const job = () => CRAFTER_JOBS.find((item) => item.id === props.jobId)!
  const progress = () => props.state.jobs[props.jobId]
  const manipulationReady = () => jobCanUseManipulation(progress())
  const safeLevel = () => clampNumber(progress().level, 1, MAX_CRAFTER_LEVEL)

  return (
    <div class="grid gap-2 border-b px-3 py-2 last:border-b-0 md:grid-cols-[7rem_minmax(12rem,1fr)_5rem_7rem] md:items-center">
      <div class="truncate text-sm font-medium">{job().name}</div>

      <RangeField
        label={`${job().name}等级`}
        value={safeLevel()}
        min={1}
        max={MAX_CRAFTER_LEVEL}
        onInput={(value) => props.onLevelChange(props.jobId, value)}
      />

      <label class="sr-only" for={`${props.jobId}-level`}>{job().name}等级</label>
      <Input
        id={`${props.jobId}-level`}
        type="number"
        value={safeLevel()}
        min={1}
        max={MAX_CRAFTER_LEVEL}
        onInput={(event) => props.onLevelChange(props.jobId, numericValue(event.currentTarget.value, safeLevel()))}
        class="h-8 px-2 text-right font-mono"
      />

      <label
        class={cx(
          'flex h-8 items-center gap-2 rounded-md border bg-background px-2 text-sm font-medium',
          manipulationReady() ? 'text-foreground' : 'text-muted-foreground',
        )}
        title={progress().level < MANIPULATION_UNLOCK_LEVEL ? `需完成 Lv.${MANIPULATION_UNLOCK_LEVEL} 职业任务` : undefined}
      >
        <input
          type="checkbox"
          checked={manipulationReady()}
          disabled={progress().level < MANIPULATION_UNLOCK_LEVEL}
          onChange={(event) => props.onManipulationChange(props.jobId, event.currentTarget.checked)}
          class="h-4 w-4 accent-foreground disabled:opacity-50"
        />
        <span>掌握</span>
      </label>
    </div>
  )
}

function JobsPanel(props: {
  state: CharacterState
  onLevelChange: (jobId: CrafterJobId, level: number) => void
  onManipulationChange: (jobId: CrafterJobId, enabled: boolean) => void
}) {
  return (
    <section class="overflow-hidden rounded-md border bg-card">
      <div class="hidden border-b bg-muted/40 px-3 py-2 text-xs font-medium text-muted-foreground md:grid md:grid-cols-[7rem_minmax(12rem,1fr)_5rem_7rem]">
        <div>职业</div>
        <div>等级</div>
        <div class="text-right">数值</div>
        <div>技能</div>
      </div>
      <div>
        <For each={CRAFTER_JOBS}>
          {(job) => (
            <JobRow
              jobId={job.id}
              state={props.state}
              onLevelChange={props.onLevelChange}
              onManipulationChange={props.onManipulationChange}
            />
          )}
        </For>
      </div>
    </section>
  )
}

function GearsetSelector(props: {
  state: CharacterState
  onActivate: (id: string) => void
  onAdd: () => void
}) {
  return (
    <aside class="rounded-md border bg-card">
      <div class="flex items-center justify-between gap-2 border-b p-3">
        <div class="text-sm font-semibold">配装档案</div>
        <Button size="icon" variant="ghost" title="新增配装" aria-label="新增配装" onClick={props.onAdd}>
          <Plus class="h-4 w-4" />
        </Button>
      </div>
      <div class="p-2">
        <For each={props.state.gearsets}>
          {(gearset) => {
            const attrs = () => gearsetAttributes(gearset)
            const active = () => props.state.activeGearsetId === gearset.id
            return (
              <button
                type="button"
                class={cx(
                  'mb-1 grid w-full grid-cols-[minmax(0,1fr)_auto] gap-2 rounded-md px-3 py-2 text-left transition-colors',
                  active() ? 'bg-accent text-foreground' : 'hover:bg-accent/70',
                )}
                onClick={() => props.onActivate(gearset.id)}
              >
                <div class="min-w-0">
                  <div class="truncate text-sm font-medium">{gearset.name}</div>
                  <div class="truncate text-xs text-muted-foreground">
                    主手 {craftTypeLabel(gearsetMainHandCraftType(gearset))}
                  </div>
                </div>
                <div class="text-right text-[11px] text-muted-foreground">
                  <div>作 {formatInteger(attrs().craftsmanship)}</div>
                  <div>加 {formatInteger(attrs().control)}</div>
                  <div>CP {formatInteger(attrs().craftPoints)}</div>
                </div>
              </button>
            )
          }}
        </For>
      </div>
    </aside>
  )
}

function equippedSlotCount(gearset: CharacterGearset) {
  return Object.values(gearset.slots).filter(Boolean).length
}

function materiaCount(gearset: CharacterGearset) {
  return Object.values(gearset.slots).reduce((count, piece) => count + (piece?.materia.length ?? 0), 0)
}

function GearsetOverviewCard(props: {
  gearset: CharacterGearset
  active: boolean
  onOpen: () => void
}) {
  const attrs = () => gearsetAttributes(props.gearset)

  return (
    <button
      type="button"
      class={cx(
        'grid min-h-36 gap-3 rounded-md border bg-card p-4 text-left transition-colors hover:bg-accent/60',
        props.active && 'border-foreground/30 bg-accent/70',
      )}
      onClick={props.onOpen}
    >
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <div class="truncate text-base font-semibold">{props.gearset.name}</div>
          <div class="mt-1 text-xs text-muted-foreground">主手 {craftTypeLabel(gearsetMainHandCraftType(props.gearset))}</div>
        </div>
        <div class="rounded border bg-background px-2 py-1 text-xs text-muted-foreground">
          {equippedSlotCount(props.gearset)} / {CRAFTER_GEAR_SLOTS.length}
        </div>
      </div>

      <div class="grid grid-cols-3 gap-2 text-xs">
        <div>
          <div class="text-muted-foreground">作业</div>
          <div class="mt-1 font-mono text-sm font-semibold">{formatInteger(attrs().craftsmanship)}</div>
        </div>
        <div>
          <div class="text-muted-foreground">加工</div>
          <div class="mt-1 font-mono text-sm font-semibold">{formatInteger(attrs().control)}</div>
        </div>
        <div>
          <div class="text-muted-foreground">CP</div>
          <div class="mt-1 font-mono text-sm font-semibold">{formatInteger(attrs().craftPoints)}</div>
        </div>
      </div>

      <div class="text-xs text-muted-foreground">魔晶石 {materiaCount(props.gearset)}</div>
    </button>
  )
}

function GearsetsOverview(props: {
  state: CharacterState
  onOpen: (id: string) => void
  onAdd: () => void
}) {
  return (
    <section class="space-y-4">
      <div class="flex items-center justify-between gap-3">
        <div class="text-sm font-semibold">配装档案</div>
        <Button size="sm" variant="outline" onClick={props.onAdd}>
          <Plus class="h-3.5 w-3.5" />
          新增
        </Button>
      </div>
      <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        <For each={props.state.gearsets}>
          {(gearset) => (
            <GearsetOverviewCard
              gearset={gearset}
              active={props.state.activeGearsetId === gearset.id}
              onOpen={() => props.onOpen(gearset.id)}
            />
          )}
        </For>
      </div>
    </section>
  )
}

function materiaTotals(piece: CharacterEquipmentPiece): GearAttributes {
  return piece.materia.reduce<GearAttributes>(
    (total, materia) => ({
      ...total,
      [materia.stat]: total[materia.stat] + materia.value,
    }),
    { craftsmanship: 0, control: 0, craftPoints: 0 },
  )
}

function normalMateriaSlots(piece: CharacterEquipmentPiece) {
  return Math.min(piece.materiaSlotCount ?? 0, 5)
}

function maxMateriaSlots(piece: CharacterEquipmentPiece) {
  return piece.advancedMelding ? 5 : normalMateriaSlots(piece)
}

function StatFormula(props: { base: number; bonus: number }) {
  return (
    <span class="font-mono">
      {formatInteger(props.base)}
      <Show when={props.bonus > 0}>
        <span class="text-emerald-700">+{formatInteger(props.bonus)}</span>
      </Show>
    </span>
  )
}

function GearsetHeader(props: {
  gearset: CharacterGearset
  count: number
  onNameChange: (name: string) => void
  onDuplicate: () => void
  onDelete: () => void
}) {
  const attrs = () => gearsetAttributes(props.gearset)

  return (
    <section class="overflow-hidden rounded-md border bg-card">
      <div class="flex flex-col gap-3 border-b p-3 lg:flex-row lg:items-end lg:justify-between">
        <label class="grid min-w-0 flex-1 gap-1 text-xs font-medium text-muted-foreground">
          配装名称
          <Input
            value={props.gearset.name}
            onInput={(event) => props.onNameChange(event.currentTarget.value)}
            class="h-9 max-w-xl text-base font-semibold"
          />
        </label>
        <div class="flex flex-wrap gap-2">
          <Button size="sm" variant="outline" onClick={props.onDuplicate}>
            <CopyPlus class="h-3.5 w-3.5" />
            复制
          </Button>
          <Button size="sm" variant="outline" disabled={props.count <= 1} onClick={props.onDelete}>
            <Trash2 class="h-3.5 w-3.5" />
            删除
          </Button>
        </div>
      </div>
      <div class="grid grid-cols-3 divide-x text-sm">
        <div class="px-3 py-2">
          <span class="text-xs text-muted-foreground">作业精度</span>
          <span class="ml-2 font-mono font-semibold">{formatInteger(attrs().craftsmanship)}</span>
        </div>
        <div class="px-3 py-2">
          <span class="text-xs text-muted-foreground">加工精度</span>
          <span class="ml-2 font-mono font-semibold">{formatInteger(attrs().control)}</span>
        </div>
        <div class="px-3 py-2">
          <span class="text-xs text-muted-foreground">制作力</span>
          <span class="ml-2 font-mono font-semibold">{formatInteger(attrs().craftPoints)}</span>
        </div>
      </div>
    </section>
  )
}

function MateriaEditor(props: {
  piece: CharacterEquipmentPiece
  onAdd: (overmeld: boolean) => void
  onUpdate: (materiaId: string, patch: Partial<CharacterMateria>) => void
  onRemove: (materiaId: string) => void
}) {
  const maxSlots = () => maxMateriaSlots(props.piece)
  const normalSlots = () => normalMateriaSlots(props.piece)
  const slotIndexes = () => Array.from({ length: maxSlots() }, (_, index) => index)

  return (
    <div class="flex min-w-0 flex-wrap items-center gap-1.5">
      <For each={slotIndexes()}>
        {(slotIndex) => {
          const materia = () => props.piece.materia[slotIndex]
          const overmeld = () => slotIndex >= normalSlots()
          return (
            <Show
              when={materia()}
              fallback={
                <button
                  type="button"
                  class={cx(
                    'flex h-8 min-w-12 items-center justify-center rounded border border-dashed bg-background px-2 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground',
                    overmeld() && 'border-amber-300 bg-amber-50/60',
                  )}
                  title={overmeld() ? '添加禁断魔晶石' : '添加魔晶石'}
                  aria-label={overmeld() ? '添加禁断魔晶石' : '添加魔晶石'}
                  onClick={() => props.onAdd(overmeld())}
                >
                  <Plus class="h-3.5 w-3.5" />
                  <span class="ml-1">{overmeld() ? '禁断' : '空槽'}</span>
                </button>
              }
            >
              {(item) => (
                <div
                  class={cx(
                    'grid h-8 grid-cols-[4.4rem_4rem_1.75rem] items-center overflow-hidden rounded border bg-background',
                    overmeld() && 'border-amber-200 bg-amber-50/70',
                  )}
                >
                  <select
                    value={item().stat}
                    title={overmeld() ? '禁断魔晶石属性' : '魔晶石属性'}
                    onChange={(event) => props.onUpdate(item().id, { stat: event.currentTarget.value as MateriaStat })}
                    class="h-8 min-w-0 border-0 bg-transparent px-2 text-xs focus-visible:outline-none"
                  >
                    <For each={MATERIA_STATS}>
                      {(stat) => <option value={stat.id}>{stat.shortLabel}</option>}
                    </For>
                  </select>
                  <Input
                    type="number"
                    value={item().value}
                    min={0}
                    max={item().stat === 'craftPoints' ? 999 : 9999}
                    title={overmeld() ? '禁断魔晶石数值' : '魔晶石数值'}
                    onInput={(event) => props.onUpdate(item().id, {
                      value: clampNumber(numericValue(event.currentTarget.value, item().value), 0, item().stat === 'craftPoints' ? 999 : 9999),
                    })}
                    class="h-8 rounded-none border-y-0 border-l px-1.5 text-right font-mono text-xs focus-visible:ring-0"
                  />
                  <button
                    type="button"
                    class="flex h-8 items-center justify-center border-l text-muted-foreground hover:bg-accent hover:text-foreground"
                    title="移除魔晶石"
                    aria-label="移除魔晶石"
                    onClick={() => props.onRemove(item().id)}
                  >
                    <X class="h-3.5 w-3.5" />
                  </button>
                </div>
              )}
            </Show>
          )
        }}
      </For>
      <Show when={maxSlots() === 0}>
        <span class="text-xs text-muted-foreground">无槽</span>
      </Show>
    </div>
  )
}

function GearSlotRow(props: {
  gearset: CharacterGearset
  slotId: GearSlotId
  active: boolean
  onOpen: (slotId: GearSlotId) => void
  onClear: (slotId: GearSlotId) => void
  onMateriaAdd: (slotId: GearSlotId, overmeld: boolean) => void
  onMateriaUpdate: (slotId: GearSlotId, materiaId: string, patch: Partial<CharacterMateria>) => void
  onMateriaRemove: (slotId: GearSlotId, materiaId: string) => void
}) {
  const slot = () => CRAFTER_GEAR_SLOTS.find((item) => item.id === props.slotId)!
  const piece = () => props.gearset.slots[props.slotId]

  return (
    <div
      class={cx(
        'grid gap-2 border-b px-3 py-2 last:border-b-0 md:grid-cols-[5rem_minmax(12rem,1fr)_7rem_13rem_minmax(18rem,1.2fr)_4.5rem] md:items-center',
        props.active && 'bg-accent/60',
      )}
    >
      <div class="text-xs font-semibold text-muted-foreground">{slot().label}</div>

      <button
        type="button"
        class="min-w-0 text-left"
        onClick={() => props.onOpen(props.slotId)}
      >
        <Show
          when={piece()}
          fallback={<span class="text-sm font-medium text-muted-foreground">选择装备</span>}
        >
          {(item) => (
            <>
              <div class="truncate text-sm font-medium">{item().name}</div>
              <div class="truncate text-xs text-muted-foreground">
                #{item().itemId ?? '-'} · Lv.{item().equipLevel ?? '-'} · {craftTypeLabel(item().craftType)}
              </div>
            </>
          )}
        </Show>
      </button>

      <div class="font-mono text-xs text-muted-foreground">
        <Show when={piece()} fallback="-">
          {(item) => <>il{item().itemLevel ?? '-'}</>}
        </Show>
      </div>

      <Show when={piece()} fallback={<div class="text-xs text-muted-foreground">作 - / 加 - / CP -</div>}>
        {(item) => {
          const materia = () => materiaTotals(item())
          return (
            <div class="grid grid-cols-3 gap-2 text-xs">
              <div>作 <StatFormula base={item().craftsmanship} bonus={materia().craftsmanship} /></div>
              <div>加 <StatFormula base={item().control} bonus={materia().control} /></div>
              <div>CP <StatFormula base={item().craftPoints} bonus={materia().craftPoints} /></div>
            </div>
          )
        }}
      </Show>

      <Show when={piece()} fallback={<div class="text-xs text-muted-foreground">选装备后镶嵌</div>}>
        {(item) => (
          <MateriaEditor
            piece={item()}
            onAdd={(overmeld) => props.onMateriaAdd(props.slotId, overmeld)}
            onUpdate={(materiaId, patch) => props.onMateriaUpdate(props.slotId, materiaId, patch)}
            onRemove={(materiaId) => props.onMateriaRemove(props.slotId, materiaId)}
          />
        )}
      </Show>

      <div class="flex items-center justify-end gap-1">
        <Button size="icon" variant="ghost" title="选择装备" aria-label="选择装备" onClick={() => props.onOpen(props.slotId)}>
          <PackageSearch class="h-4 w-4" />
        </Button>
        <Show when={piece()}>
          <Button size="icon" variant="ghost" title="清空装备" aria-label="清空装备" onClick={() => props.onClear(props.slotId)}>
            <X class="h-4 w-4" />
          </Button>
        </Show>
      </div>
    </div>
  )
}

function GearSlotsTable(props: {
  gearset: CharacterGearset
  activeSlotId?: GearSlotId
  onOpenSlot: (slotId: GearSlotId) => void
  onClearSlot: (slotId: GearSlotId) => void
  onMateriaAdd: (slotId: GearSlotId, overmeld: boolean) => void
  onMateriaUpdate: (slotId: GearSlotId, materiaId: string, patch: Partial<CharacterMateria>) => void
  onMateriaRemove: (slotId: GearSlotId, materiaId: string) => void
}) {
  return (
    <section class="overflow-hidden rounded-md border bg-card">
      <div class="hidden border-b bg-muted/40 px-3 py-2 text-xs font-medium text-muted-foreground md:grid md:grid-cols-[5rem_minmax(12rem,1fr)_7rem_13rem_minmax(18rem,1.2fr)_4.5rem]">
        <div>槽位</div>
        <div>装备</div>
        <div>品级</div>
        <div>属性</div>
        <div>魔晶石</div>
        <div class="text-right">操作</div>
      </div>
      <For each={CRAFTER_GEAR_SLOTS}>
        {(slot) => (
          <GearSlotRow
            gearset={props.gearset}
            slotId={slot.id}
            active={props.activeSlotId === slot.id}
            onOpen={props.onOpenSlot}
            onClear={props.onClearSlot}
            onMateriaAdd={props.onMateriaAdd}
            onMateriaUpdate={props.onMateriaUpdate}
            onMateriaRemove={props.onMateriaRemove}
          />
        )}
      </For>
    </section>
  )
}

function EquipmentPickerPanel(props: {
  picker?: ActiveEquipmentPicker
  gearset?: CharacterGearset
  data?: CraftDataPackage
  loading?: boolean
  onSelect: (piece: CharacterEquipmentPiece) => void
  onClose: () => void
}) {
  const [query, setQuery] = createSignal('')
  const [minimumItemLevel, setMinimumItemLevel] = createSignal(0)
  const slot = () => CRAFTER_GEAR_SLOTS.find((item) => item.id === props.picker?.slotId)
  const candidates = createMemo(() => {
    const picker = props.picker
    if (!picker) return []
    const needle = query().trim().toLocaleLowerCase()
    const minItemLevel = minimumItemLevel()
    const dataSlotId = equipmentDataSlotId(picker.slotId)
    return (props.data?.equipment ?? [])
      .filter((item) => item.slotId === dataSlotId)
      .filter((item) => !needle || item.name.toLocaleLowerCase().includes(needle) || String(item.itemId).includes(needle))
      .filter((item) => item.itemLevel >= minItemLevel)
  })
  const selectedItemId = () => {
    const picker = props.picker
    if (!picker) return undefined
    return props.gearset?.slots[picker.slotId]?.itemId
  }

  return (
    <Show when={props.picker && slot()}>
      {(value) => (
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4" role="dialog" aria-modal="true" onClick={props.onClose}>
          <section class="flex max-h-[86vh] w-full max-w-5xl flex-col overflow-hidden rounded-md border bg-card shadow-xl" onClick={(event) => event.stopPropagation()}>
            <div class="grid gap-3 border-b p-4 lg:grid-cols-[minmax(0,1fr)_minmax(14rem,22rem)_8rem_auto] lg:items-end">
              <div>
                <div class="text-base font-semibold">选择装备 · {value().label}</div>
                <div class="mt-1 text-xs text-muted-foreground">
                  显示 {formatInteger(candidates().length)} 件
                </div>
              </div>
              <label class="grid gap-1 text-xs font-medium text-muted-foreground">
                搜索
                <Input
                  value={query()}
                  placeholder="装备名或物品 ID"
                  onInput={(event) => setQuery(event.currentTarget.value)}
                  class="h-8"
                />
              </label>
              <label class="grid gap-1 text-xs font-medium text-muted-foreground">
                最低品级
                <Input
                  type="number"
                  value={minimumItemLevel()}
                  min={0}
                  max={9999}
                  onInput={(event) => setMinimumItemLevel(clampNumber(numericValue(event.currentTarget.value), 0, 9999))}
                  class="h-8"
                />
              </label>
              <Button size="icon" variant="ghost" title="关闭选择列表" aria-label="关闭选择列表" onClick={props.onClose}>
                <X class="h-4 w-4" />
              </Button>
            </div>

            <div class="min-h-0 flex-1 overflow-hidden">
              <Show
                when={!props.loading}
                fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="正在读取装备库" />}
              >
                <Show
                  when={candidates().length > 0}
                  fallback={<EmptyState icon={<PackageSearch class="h-6 w-6" />} title="没有匹配装备" />}
                >
                  <div class="hidden border-b bg-muted/40 px-3 py-2 text-xs font-medium text-muted-foreground md:grid md:grid-cols-[minmax(14rem,1fr)_5rem_7rem_13rem_7rem]">
                    <div>装备</div>
                    <div>品级</div>
                    <div>职业</div>
                    <div>属性</div>
                    <div>镶嵌</div>
                  </div>
                  <div class="max-h-[62vh] overflow-y-auto">
                    <For each={candidates()}>
                      {(item) => {
                        const selected = () => item.itemId === selectedItemId()
                        return (
                          <button
                            type="button"
                            class={cx(
                              'grid w-full gap-2 border-b px-3 py-2 text-left transition-colors last:border-b-0 hover:bg-accent md:grid-cols-[minmax(14rem,1fr)_5rem_7rem_13rem_7rem] md:items-center',
                              selected() && 'bg-accent/70',
                            )}
                            onClick={() => props.onSelect(equipmentPieceFromItem(item, props.picker!.slotId))}
                          >
                            <div class="min-w-0">
                              <div class="truncate text-sm font-medium">{item.name}</div>
                              <div class="truncate text-xs text-muted-foreground">#{item.itemId} · Lv.{item.equipLevel}</div>
                            </div>
                            <div class="font-mono text-xs">il{item.itemLevel}</div>
                            <div class="text-xs text-muted-foreground">{craftTypeLabel(item.craftType)}</div>
                            <div class="grid grid-cols-3 gap-2 text-xs">
                              <span>作 {formatInteger(item.craftsmanship)}</span>
                              <span>加 {formatInteger(item.control)}</span>
                              <span>CP {formatInteger(item.craftPoints)}</span>
                            </div>
                            <div class="text-xs text-muted-foreground">
                              {item.materiaSlotCount || 0}
                              {item.advancedMelding ? ' + 禁断' : ''}
                            </div>
                          </button>
                        )
                      }}
                    </For>
                  </div>
                </Show>
              </Show>
            </div>
          </section>
        </div>
      )}
    </Show>
  )
}

function GearsetsPanel(props: {
  state: CharacterState
  onActivate: (id: string) => void
  onAdd: () => void
  onUpdateGearset: (gearsetId: string, patch: Partial<CharacterGearset>) => void
  onDuplicate: (gearsetId: string) => void
  onDelete: (gearsetId: string) => void
  onOpenSlot: (gearsetId: string, slotId: GearSlotId) => void
  onClearSlot: (gearsetId: string, slotId: GearSlotId) => void
  onMateriaAdd: (gearsetId: string, slotId: GearSlotId, overmeld: boolean) => void
  onMateriaUpdate: (gearsetId: string, slotId: GearSlotId, materiaId: string, patch: Partial<CharacterMateria>) => void
  onMateriaRemove: (gearsetId: string, slotId: GearSlotId, materiaId: string) => void
  picker?: ActiveEquipmentPicker
  editingGearsetId?: string
  onOpenGearset: (id: string) => void
  onBackToOverview: () => void
}) {
  const editingGearset = createMemo(() => props.state.gearsets.find((gearset) => gearset.id === props.editingGearsetId))

  return (
    <Show
      when={editingGearset()}
      fallback={<GearsetsOverview state={props.state} onOpen={props.onOpenGearset} onAdd={props.onAdd} />}
    >
      {(editing) => (
        <div class="grid gap-4 xl:grid-cols-[260px_minmax(0,1fr)]">
          <div class="space-y-4">
            <Button size="sm" variant="outline" onClick={props.onBackToOverview}>
              <ArrowLeft class="h-3.5 w-3.5" />
              配装列表
            </Button>
            <GearsetSelector
              state={props.state}
              onActivate={(id) => {
                props.onActivate(id)
                props.onOpenGearset(id)
              }}
              onAdd={props.onAdd}
            />

            <GearsetHeader
              gearset={editing()}
              count={props.state.gearsets.length}
              onNameChange={(name) => props.onUpdateGearset(editing().id, { name })}
              onDuplicate={() => props.onDuplicate(editing().id)}
              onDelete={() => props.onDelete(editing().id)}
            />
          </div>

          <section class="space-y-4">
            <GearSlotsTable
              gearset={editing()}
              activeSlotId={props.picker?.gearsetId === editing().id ? props.picker.slotId : undefined}
              onOpenSlot={(slotId) => props.onOpenSlot(editing().id, slotId)}
              onClearSlot={(slotId) => props.onClearSlot(editing().id, slotId)}
              onMateriaAdd={(slotId, overmeld) => props.onMateriaAdd(editing().id, slotId, overmeld)}
              onMateriaUpdate={(slotId, materiaId, patch) => props.onMateriaUpdate(editing().id, slotId, materiaId, patch)}
              onMateriaRemove={(slotId, materiaId) => props.onMateriaRemove(editing().id, slotId, materiaId)}
            />
          </section>
        </div>
      )}
    </Show>
  )
}

export default function CharacterPage() {
  const [state, setState] = createSignal(loadCharacterState())
  const [tab, setTab] = createSignal<CharacterTab>('jobs')
  const [editingGearsetId, setEditingGearsetId] = createSignal<string | undefined>()
  const [picker, setPicker] = createSignal<ActiveEquipmentPicker | undefined>()
  const [shouldLoadCraftData, setShouldLoadCraftData] = createSignal(false)
  const [craftData] = createResource(shouldLoadCraftData, (enabled) => enabled ? loadCraftData() : undefined)
  const pickerGearset = createMemo(() => state().gearsets.find((gearset) => gearset.id === picker()?.gearsetId))

  createEffect(() => {
    saveCharacterState(state())
  })

  const setJobLevel = (jobId: CrafterJobId, level: number) => {
    updateState(setState, (current) => {
      const nextLevel = clampNumber(level, 1, MAX_CRAFTER_LEVEL)
      const currentJob = current.jobs[jobId]
      current.jobs[jobId] = {
        level: nextLevel,
        questLevel: Math.min(currentJob.questLevel, nextLevel),
      }
      return current
    })
  }

  const setJobManipulation = (jobId: CrafterJobId, enabled: boolean) => {
    updateState(setState, (current) => {
      const currentJob = current.jobs[jobId]
      const unlockLevel = Math.min(MANIPULATION_UNLOCK_LEVEL, currentJob.level)
      current.jobs[jobId] = {
        ...currentJob,
        questLevel: enabled ? unlockLevel : Math.min(currentJob.level, MANIPULATION_UNLOCK_LEVEL - 1),
      }
      return current
    })
  }

  const activateGearset = (gearsetId: string) => {
    updateState(setState, (current) => ({ ...current, activeGearsetId: gearsetId }))
  }

  const openGearsetDetail = (gearsetId: string) => {
    activateGearset(gearsetId)
    setEditingGearsetId(gearsetId)
    setPicker(undefined)
  }

  const backToGearsetsOverview = () => {
    setEditingGearsetId(undefined)
    setPicker(undefined)
  }

  const addGearset = () => {
    let newGearsetId: string | undefined
    updateState(setState, (current) => {
      const gearset = createCharacterGearset(`配装 ${current.gearsets.length + 1}`)
      newGearsetId = gearset.id
      return {
        ...current,
        gearsets: [...current.gearsets, gearset],
        activeGearsetId: gearset.id,
      }
    })
    setTab('gearsets')
    setEditingGearsetId(newGearsetId)
    setPicker(undefined)
  }

  const updateGearset = (gearsetId: string, patch: Partial<CharacterGearset>) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => gearset.id === gearsetId ? { ...gearset, ...patch } : gearset),
    }))
  }

  const duplicateGearset = (gearsetId: string) => {
    let copiedGearsetId: string | undefined
    updateState(setState, (current) => {
      const source = current.gearsets.find((gearset) => gearset.id === gearsetId)
      if (!source) return current
      const copy: CharacterGearset = {
        ...source,
        id: id('gearset'),
        name: `${source.name} 副本`,
        manualAttributes: { ...source.manualAttributes },
        baseAttributes: { ...source.baseAttributes },
        slots: cloneSlots(source.slots),
      }
      copiedGearsetId = copy.id
      return {
        ...current,
        gearsets: [...current.gearsets, copy],
        activeGearsetId: copy.id,
      }
    })
    setEditingGearsetId(copiedGearsetId)
    setPicker(undefined)
  }

  const deleteGearset = (gearsetId: string) => {
    updateState(setState, (current) => {
      if (current.gearsets.length <= 1) return current
      const gearsets = current.gearsets.filter((gearset) => gearset.id !== gearsetId)
      if (editingGearsetId() === gearsetId) setEditingGearsetId(undefined)
      if (picker()?.gearsetId === gearsetId) setPicker(undefined)
      return {
        ...current,
        gearsets,
        activeGearsetId: current.activeGearsetId === gearsetId ? gearsets[0].id : current.activeGearsetId,
      }
    })
  }

  const openSlot = (gearsetId: string, slotId: GearSlotId) => {
    setShouldLoadCraftData(true)
    setPicker({ gearsetId, slotId })
  }

  const clearSlot = (gearsetId: string, slotId: GearSlotId) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => {
        if (gearset.id !== gearsetId) return gearset
        const slots = { ...gearset.slots }
        delete slots[slotId]
        return { ...gearset, slots }
      }),
    }))
  }

  const selectEquipment = (value: ActiveEquipmentPicker, piece: CharacterEquipmentPiece) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => (
        gearset.id === value.gearsetId
          ? {
              ...gearset,
              slots: { ...gearset.slots, [value.slotId]: piece },
            }
          : gearset
      )),
    }))
    setPicker(undefined)
  }

  const addMateria = (gearsetId: string, slotId: GearSlotId, overmeld: boolean) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => {
        if (gearset.id !== gearsetId) return gearset
        const piece = gearset.slots[slotId]
        if (!piece) return gearset
        const maxSlots = maxMateriaSlots(piece)
        if (piece.materia.length >= maxSlots) return gearset
        const normalSlots = normalMateriaSlots(piece)
        return {
          ...gearset,
          slots: {
            ...gearset.slots,
            [slotId]: {
              ...piece,
              materia: [
                ...piece.materia,
                {
                  id: id('materia'),
                  stat: 'craftsmanship',
                  value: 0,
                  overmeld: overmeld || piece.materia.length >= normalSlots,
                },
              ],
            },
          },
        }
      }),
    }))
  }

  const updateMateria = (gearsetId: string, slotId: GearSlotId, materiaId: string, patch: Partial<CharacterMateria>) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => {
        if (gearset.id !== gearsetId) return gearset
        const piece = gearset.slots[slotId]
        if (!piece) return gearset
        return {
          ...gearset,
          slots: {
            ...gearset.slots,
            [slotId]: {
              ...piece,
              materia: piece.materia.map((materia) => materia.id === materiaId ? { ...materia, ...patch } : materia),
            },
          },
        }
      }),
    }))
  }

  const removeMateria = (gearsetId: string, slotId: GearSlotId, materiaId: string) => {
    updateState(setState, (current) => ({
      ...current,
      gearsets: current.gearsets.map((gearset) => {
        if (gearset.id !== gearsetId) return gearset
        const piece = gearset.slots[slotId]
        if (!piece) return gearset
        return {
          ...gearset,
          slots: {
            ...gearset.slots,
            [slotId]: {
              ...piece,
              materia: piece.materia.filter((materia) => materia.id !== materiaId),
            },
          },
        }
      }),
    }))
  }

  const reset = () => {
    setState(createDefaultCharacterState())
    setPicker(undefined)
  }

  return (
    <div class="min-h-screen bg-background">
      <div class="border-b bg-background px-4 py-4 sm:px-6 lg:px-8">
        <div class="mx-auto flex max-w-[1500px] flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <div class="text-sm text-muted-foreground">数据 / 角色</div>
            <h1 class="text-2xl font-semibold">角色</h1>
          </div>
          <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between xl:justify-end">
            <SegmentedTabs value={tab()} onChange={setTab} />
            <div class="flex flex-wrap items-center gap-2">
              <Button size="sm" variant="outline" onClick={reset}>
                <RotateCcw class="h-3.5 w-3.5" />
                重置
              </Button>
            </div>
          </div>
        </div>
      </div>

      <div class="mx-auto max-w-[1500px] px-4 py-6 sm:px-6 lg:px-8">
        <Show
          when={tab() === 'jobs'}
          fallback={
            <GearsetsPanel
              state={state()}
              onActivate={activateGearset}
              onAdd={addGearset}
              onUpdateGearset={updateGearset}
              onDuplicate={duplicateGearset}
              onDelete={deleteGearset}
              onOpenSlot={openSlot}
              onClearSlot={clearSlot}
              onMateriaAdd={addMateria}
              onMateriaUpdate={updateMateria}
              onMateriaRemove={removeMateria}
              picker={picker()}
              editingGearsetId={editingGearsetId()}
              onOpenGearset={openGearsetDetail}
              onBackToOverview={backToGearsetsOverview}
            />
          }
        >
          <JobsPanel state={state()} onLevelChange={setJobLevel} onManipulationChange={setJobManipulation} />
        </Show>
      </div>

      <EquipmentPickerPanel
        picker={picker()}
        gearset={pickerGearset()}
        data={craftData()}
        loading={craftData.loading}
        onSelect={(piece) => {
          const currentPicker = picker()
          if (currentPicker) selectEquipment(currentPicker, piece)
        }}
        onClose={() => setPicker(undefined)}
      />
    </div>
  )
}
