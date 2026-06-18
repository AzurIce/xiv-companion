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
  { id: 'notes', label: '笔记', href: '/notes', group: 'tools', status: 'ready' },
]

export function moduleGroupLabel(group: AppModule['group']): string {
  if (group === 'tools') return '工具'
  if (group === 'preview') return '预览'
  return '数据'
}
