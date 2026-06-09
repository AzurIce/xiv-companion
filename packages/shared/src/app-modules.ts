export type ModuleStatus = 'ready' | 'planned'

export interface AppModule {
  id: string
  label: string
  href: string
  group: 'tools' | 'preview' | 'data'
  status: ModuleStatus
}

export const appModules: AppModule[] = [
  { id: 'crafting', label: '合成检索', href: '/crafting', group: 'tools', status: 'ready' },
  { id: 'glamour', label: '幻化预览', href: '/glamour', group: 'preview', status: 'planned' },
  { id: 'housing', label: '家具预览', href: '/housing', group: 'preview', status: 'planned' },
  { id: 'library', label: '资料库', href: '/library', group: 'data', status: 'planned' },
]

export function moduleGroupLabel(group: AppModule['group']): string {
  if (group === 'tools') return '工具'
  if (group === 'preview') return '预览'
  return '数据'
}

