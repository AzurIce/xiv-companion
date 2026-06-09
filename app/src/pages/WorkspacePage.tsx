import { For } from 'solid-js'
import { A } from '@solidjs/router'
import { Boxes, Database, Shirt, Sofa, Wrench } from 'lucide-solid'
import { appModules, cx } from '@xiv-companian/shared'
import { Badge, Card, CardContent, CardHeader, CardTitle } from '@xiv-companian/ui'

function iconFor(id: string) {
  if (id === 'crafting') return <Wrench class="h-5 w-5" />
  if (id === 'glamour') return <Shirt class="h-5 w-5" />
  if (id === 'housing') return <Sofa class="h-5 w-5" />
  if (id === 'library') return <Database class="h-5 w-5" />
  return <Boxes class="h-5 w-5" />
}

export default function WorkspacePage() {
  return (
    <div class="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8">
      <div class="flex flex-col gap-2">
        <div class="text-sm text-muted-foreground">工作台</div>
        <h1 class="text-2xl font-semibold">XIV Companion</h1>
      </div>

      <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <For each={appModules}>
          {(module) => {
            const enabled = module.status === 'ready'
            const body = (
              <Card class={cx('h-full transition-colors', enabled && 'hover:border-foreground/20')}>
                <CardHeader>
                  <div class="flex items-center justify-between gap-3">
                    <div class="flex h-10 w-10 items-center justify-center rounded-lg border bg-background text-muted-foreground">
                      {iconFor(module.id)}
                    </div>
                    <Badge variant={enabled ? 'success' : 'outline'}>
                      {enabled ? '可用' : '计划'}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent class="space-y-2">
                  <CardTitle>{module.label}</CardTitle>
                  <div class="text-sm text-muted-foreground">
                    {module.id === 'crafting' && '配方树、素材汇总、来源选择'}
                    {module.id === 'glamour' && '装备外观与染色工作区'}
                    {module.id === 'housing' && '庭院与室内家具模型工作区'}
                    {module.id === 'library' && '物品、分类、资源索引'}
                  </div>
                </CardContent>
              </Card>
            )

            return enabled ? (
              <A href={module.href} class="block">
                {body}
              </A>
            ) : (
              <div>{body}</div>
            )
          }}
        </For>
      </div>
    </div>
  )
}

