import type { CrafterAttributes, RaphaelSolveOptions } from './crafting'

export const CHARACTER_STORAGE_KEY = 'xiv-companion-character-v1'
export const MAX_CRAFTER_LEVEL = 100
export const QUALITY_ASSURANCE_UNLOCK_LEVEL = 63
export const MANIPULATION_UNLOCK_LEVEL = 65

export const CRAFTER_QUEST_UNLOCKS = [
  {
    id: 'qualityAssurance',
    level: QUALITY_ASSURANCE_UNLOCK_LEVEL,
    name: 'Quality Assurance',
    label: '品质保证',
    kind: 'trait',
    solverSupport: 'tracked',
  },
  {
    id: 'manipulation',
    level: MANIPULATION_UNLOCK_LEVEL,
    name: 'Manipulation',
    label: '掌握',
    kind: 'action',
    solverSupport: 'raphaelOption',
  },
] as const

export const CRAFTER_JOBS = [
  { id: 'carpenter', craftType: 0, name: '刻木匠', shortName: '木工' },
  { id: 'blacksmith', craftType: 1, name: '锻铁匠', shortName: '锻冶' },
  { id: 'armorer', craftType: 2, name: '铸甲匠', shortName: '甲胄' },
  { id: 'goldsmith', craftType: 3, name: '雕金匠', shortName: '雕金' },
  { id: 'leatherworker', craftType: 4, name: '制革匠', shortName: '皮革' },
  { id: 'weaver', craftType: 5, name: '裁衣匠', shortName: '裁缝' },
  { id: 'alchemist', craftType: 6, name: '炼金术士', shortName: '炼金' },
  { id: 'culinarian', craftType: 7, name: '烹调师', shortName: '烹调' },
] as const

export type CrafterJobId = typeof CRAFTER_JOBS[number]['id']
export type CrafterQuestUnlockId = typeof CRAFTER_QUEST_UNLOCKS[number]['id']

export const CRAFTER_GEAR_SLOTS = [
  { id: 'mainHand', label: '主手', group: 'tools' },
  { id: 'offHand', label: '副手', group: 'tools' },
  { id: 'head', label: '头部', group: 'armor' },
  { id: 'body', label: '身体', group: 'armor' },
  { id: 'hands', label: '手部', group: 'armor' },
  { id: 'legs', label: '腿部', group: 'armor' },
  { id: 'feet', label: '脚部', group: 'armor' },
  { id: 'ears', label: '耳饰', group: 'accessories' },
  { id: 'neck', label: '项链', group: 'accessories' },
  { id: 'wrists', label: '手镯', group: 'accessories' },
  { id: 'ringRight', label: '戒指 1', group: 'accessories' },
  { id: 'ringLeft', label: '戒指 2', group: 'accessories' },
] as const

export const RING_EQUIPMENT_SLOT_ID = 'ring'

export type GearSlotId = typeof CRAFTER_GEAR_SLOTS[number]['id']
export type GearSlotGroup = typeof CRAFTER_GEAR_SLOTS[number]['group']
export type GearsetStatMode = 'manual' | 'equipment'
export type MateriaStat = keyof GearAttributes

export const MATERIA_STATS = [
  { id: 'craftsmanship', label: '作业精度', shortLabel: '作' },
  { id: 'control', label: '加工精度', shortLabel: '加' },
  { id: 'craftPoints', label: '制作力', shortLabel: 'CP' },
] as const satisfies Array<{ id: MateriaStat; label: string; shortLabel: string }>

export interface GearAttributes {
  craftsmanship: number
  control: number
  craftPoints: number
}

export interface CrafterJobProgress {
  level: number
  questLevel: number
}

export interface CharacterEquipmentPiece extends GearAttributes {
  id: string
  itemId?: number
  slotId: GearSlotId
  name: string
  patch?: string
  itemLevel?: number
  equipLevel?: number
  craftType?: number
  setName?: string
  materiaSlotCount?: number
  advancedMelding?: boolean
  materia: CharacterMateria[]
}

export interface CharacterMateria {
  id: string
  stat: MateriaStat
  value: number
  overmeld?: boolean
}

export interface CharacterGearset {
  id: string
  name: string
  statMode: GearsetStatMode
  manualAttributes: GearAttributes
  baseAttributes: GearAttributes
  slots: Partial<Record<GearSlotId, CharacterEquipmentPiece>>
}

export interface CharacterState {
  version: 1
  jobs: Record<CrafterJobId, CrafterJobProgress>
  gearsets: CharacterGearset[]
  activeGearsetId: string
}

export const DEFAULT_GEAR_ATTRIBUTES: GearAttributes = {
  craftsmanship: 4900,
  control: 4800,
  craftPoints: 620,
}

export const DEFAULT_CRAFTER_ATTRIBUTES: CrafterAttributes = {
  level: MAX_CRAFTER_LEVEL,
  ...DEFAULT_GEAR_ATTRIBUTES,
}

export const DEFAULT_RAPHAEL_SOLVE_OPTIONS: RaphaelSolveOptions = {
  targetQuality: undefined,
  useManipulation: true,
  useHeartAndSoul: false,
  useQuickInnovation: false,
  useTrainedEye: true,
  backloadProgress: false,
  adversarial: false,
  stellarSteadyHandCharges: 0,
}

function id(prefix: string) {
  const value = globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
  return `${prefix}-${value}`
}

function clampInteger(value: unknown, fallback: number, min: number, max: number) {
  const parsed = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, Math.round(parsed)))
}

function stringValue(value: unknown, fallback = '') {
  return typeof value === 'string' && value.trim() ? value.trim() : fallback
}

function normalizeGearAttributes(value: unknown, fallback: GearAttributes): GearAttributes {
  const raw = isRecord(value) ? value : {}
  return {
    craftsmanship: clampInteger(raw.craftsmanship, fallback.craftsmanship, 0, 99999),
    control: clampInteger(raw.control, fallback.control, 0, 99999),
    craftPoints: clampInteger(raw.craftPoints, fallback.craftPoints, 0, 9999),
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function isGearSlotId(value: unknown): value is GearSlotId {
  return CRAFTER_GEAR_SLOTS.some((slot) => slot.id === value)
}

function normalizeStatMode(value: unknown): GearsetStatMode {
  return value === 'equipment' ? 'equipment' : 'manual'
}

function isMateriaStat(value: unknown): value is MateriaStat {
  return MATERIA_STATS.some((stat) => stat.id === value)
}

function normalizeMateria(value: unknown, index: number, normalSlotCount: number): CharacterMateria | undefined {
  if (!isRecord(value)) return undefined
  const stat = isMateriaStat(value.stat) ? value.stat : 'craftsmanship'
  const materia: CharacterMateria = {
    id: stringValue(value.id, id('materia')),
    stat,
    value: clampInteger(value.value, 0, 0, stat === 'craftPoints' ? 999 : 9999),
  }
  if (typeof value.overmeld === 'boolean') {
    materia.overmeld = value.overmeld
  } else if (index >= normalSlotCount) {
    materia.overmeld = true
  }
  return materia
}

function normalizeMateriaList(value: unknown, normalSlotCount: number, advancedMelding: boolean | undefined): CharacterMateria[] {
  if (!Array.isArray(value)) return []
  const maxSlots = advancedMelding ? 5 : Math.min(normalSlotCount, 5)
  return value
    .slice(0, maxSlots)
    .map((item, index) => normalizeMateria(item, index, normalSlotCount))
    .filter((item): item is CharacterMateria => !!item)
}

function createDefaultJobs(): Record<CrafterJobId, CrafterJobProgress> {
  return Object.fromEntries(
    CRAFTER_JOBS.map((job) => [
      job.id,
      {
        level: MAX_CRAFTER_LEVEL,
        questLevel: MAX_CRAFTER_LEVEL,
      },
    ]),
  ) as Record<CrafterJobId, CrafterJobProgress>
}

function normalizeJobProgress(value: unknown): CrafterJobProgress {
  const raw = isRecord(value) ? value : {}
  const level = clampInteger(raw.level, MAX_CRAFTER_LEVEL, 1, MAX_CRAFTER_LEVEL)
  const questLevel = clampInteger(raw.questLevel, Math.min(level, MAX_CRAFTER_LEVEL), 0, level)
  return { level, questLevel }
}

function normalizeEquipmentPiece(value: unknown, slotId: GearSlotId): CharacterEquipmentPiece | undefined {
  if (!isRecord(value)) return undefined
  const name = stringValue(value.name)
  if (!name) return undefined

  const rawCraftType = isRecord(value) && value.craftType != null ? Number(value.craftType) : undefined

  const materiaSlotCount = clampInteger(value.materiaSlotCount, 0, 0, 99) || undefined
  const advancedMelding = typeof value.advancedMelding === 'boolean' ? value.advancedMelding : undefined

  return {
    id: stringValue(value.id, id('equipment')),
    itemId: clampInteger(value.itemId, 0, 0, 999999) || undefined,
    slotId,
    name,
    patch: stringValue(value.patch) || undefined,
    itemLevel: clampInteger(value.itemLevel, 0, 0, 9999) || undefined,
    equipLevel: clampInteger(value.equipLevel, 0, 0, MAX_CRAFTER_LEVEL) || undefined,
    craftType: Number.isFinite(rawCraftType) ? clampInteger(rawCraftType, 0, 0, 99) : undefined,
    setName: stringValue(value.setName) || undefined,
    materiaSlotCount,
    advancedMelding,
    materia: normalizeMateriaList(value.materia, materiaSlotCount ?? 0, advancedMelding),
    ...normalizeGearAttributes(value, { craftsmanship: 0, control: 0, craftPoints: 0 }),
  }
}

function normalizeSlots(value: unknown): Partial<Record<GearSlotId, CharacterEquipmentPiece>> {
  const raw = isRecord(value) ? value : {}
  const slots: Partial<Record<GearSlotId, CharacterEquipmentPiece>> = {}

  for (const [slotId, piece] of Object.entries(raw)) {
    if (!isGearSlotId(slotId)) continue
    const normalized = normalizeEquipmentPiece(piece, slotId)
    if (normalized) slots[slotId] = normalized
  }

  return slots
}

export function createCharacterGearset(name = '默认配装'): CharacterGearset {
  return {
    id: id('gearset'),
    name,
    statMode: 'equipment',
    manualAttributes: { ...DEFAULT_GEAR_ATTRIBUTES },
    baseAttributes: { craftsmanship: 0, control: 0, craftPoints: 0 },
    slots: {},
  }
}

function normalizeGearset(value: unknown, index: number): CharacterGearset {
  const fallback = createCharacterGearset(index === 0 ? '默认配装' : `配装 ${index + 1}`)
  if (!isRecord(value)) return fallback

  return {
    id: stringValue(value.id, fallback.id),
    name: stringValue(value.name, fallback.name),
    statMode: normalizeStatMode(value.statMode),
    manualAttributes: normalizeGearAttributes(value.manualAttributes, DEFAULT_GEAR_ATTRIBUTES),
    baseAttributes: normalizeGearAttributes(value.baseAttributes, { craftsmanship: 0, control: 0, craftPoints: 0 }),
    slots: normalizeSlots(value.slots),
  }
}

export function createDefaultCharacterState(): CharacterState {
  const gearset = createCharacterGearset()
  return {
    version: 1,
    jobs: createDefaultJobs(),
    gearsets: [gearset],
    activeGearsetId: gearset.id,
  }
}

export function normalizeCharacterState(value: unknown): CharacterState {
  const fallback = createDefaultCharacterState()
  if (!isRecord(value)) return fallback

  const rawJobs = isRecord(value.jobs) ? value.jobs : {}
  const jobs = Object.fromEntries(
    CRAFTER_JOBS.map((job) => [job.id, normalizeJobProgress(rawJobs[job.id])]),
  ) as Record<CrafterJobId, CrafterJobProgress>

  const gearsets = (Array.isArray(value.gearsets) ? value.gearsets : [])
    .map((gearset, index) => normalizeGearset(gearset, index))
  if (gearsets.length === 0) gearsets.push(...fallback.gearsets)

  const activeGearsetId = typeof value.activeGearsetId === 'string'
    && gearsets.some((gearset) => gearset.id === value.activeGearsetId)
    ? value.activeGearsetId
    : gearsets[0].id

  return {
    version: 1,
    jobs,
    gearsets,
    activeGearsetId,
  }
}

export function loadCharacterState(): CharacterState {
  try {
    return normalizeCharacterState(JSON.parse(localStorage.getItem(CHARACTER_STORAGE_KEY) ?? 'null'))
  } catch {
    return createDefaultCharacterState()
  }
}

export function saveCharacterState(state: CharacterState) {
  localStorage.setItem(CHARACTER_STORAGE_KEY, JSON.stringify(normalizeCharacterState(state)))
}

export function jobForCraftType(craftType: number | undefined) {
  return CRAFTER_JOBS.find((job) => job.craftType === craftType) ?? CRAFTER_JOBS[0]
}

export function jobCanUseManipulation(job: CrafterJobProgress) {
  return job.level >= MANIPULATION_UNLOCK_LEVEL && job.questLevel >= MANIPULATION_UNLOCK_LEVEL
}

export function jobUnlockedQuestFeatures(job: CrafterJobProgress) {
  return CRAFTER_QUEST_UNLOCKS.filter((unlock) => job.level >= unlock.level && job.questLevel >= unlock.level)
}

export function getActiveGearset(state: CharacterState): CharacterGearset {
  return state.gearsets.find((gearset) => gearset.id === state.activeGearsetId) ?? state.gearsets[0]
}

export function getGearset(state: CharacterState, gearsetId: string | undefined): CharacterGearset {
  return state.gearsets.find((gearset) => gearset.id === gearsetId) ?? getActiveGearset(state)
}

export function gearsetEquipmentAttributes(gearset: CharacterGearset): GearAttributes {
  return Object.values(gearset.slots).reduce<GearAttributes>(
    (total, piece) => {
      if (!piece) return total
      const materia = piece.materia.reduce<GearAttributes>(
        (materiaTotal, item) => ({
          ...materiaTotal,
          [item.stat]: materiaTotal[item.stat] + item.value,
        }),
        { craftsmanship: 0, control: 0, craftPoints: 0 },
      )
      return {
        craftsmanship: total.craftsmanship + piece.craftsmanship + materia.craftsmanship,
        control: total.control + piece.control + materia.control,
        craftPoints: total.craftPoints + piece.craftPoints + materia.craftPoints,
      }
    },
    { craftsmanship: 0, control: 0, craftPoints: 0 },
  )
}

export function gearsetAttributes(gearset: CharacterGearset): GearAttributes {
  return gearsetEquipmentAttributes(gearset)
}

export function equipmentDataSlotId(slotId: GearSlotId): string {
  return slotId === 'ringLeft' || slotId === 'ringRight' ? RING_EQUIPMENT_SLOT_ID : slotId
}

export function gearsetMainHandCraftType(gearset: CharacterGearset): number | undefined {
  return gearset.slots.mainHand?.craftType
}

export function gearsetMatchesCraftType(gearset: CharacterGearset, craftType: number | undefined): boolean {
  const mainHandCraftType = gearsetMainHandCraftType(gearset)
  return mainHandCraftType == null || craftType == null || mainHandCraftType === craftType
}

export function characterRaphaelInputForCraftType(
  state: CharacterState,
  craftType: number | undefined,
  gearsetId?: string,
) {
  const job = jobForCraftType(craftType)
  const progress = state.jobs[job.id]
  const gearset = getGearset(state, gearsetId)
  const attributes = gearsetAttributes(gearset)

  return {
    job,
    progress,
    gearset,
    attrs: {
      level: progress.level,
      ...attributes,
    } satisfies CrafterAttributes,
    options: {
      ...DEFAULT_RAPHAEL_SOLVE_OPTIONS,
      useManipulation: jobCanUseManipulation(progress),
    } satisfies RaphaelSolveOptions,
  }
}
