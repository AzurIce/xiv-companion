import { createResource, For, Show } from 'solid-js'
import { A } from '@solidjs/router'
import { BookOpen, Database, UserRoundCog, Wrench } from 'lucide-solid'
import { appModules, cx, formatInteger, loadCraftData } from '../lib'
import { Card, CardContent, CardHeader, CardTitle } from '../ui'

function iconFor(id: string) {
  if (id === 'character') return <UserRoundCog class="h-5 w-5" />
  if (id === 'crafting') return <Wrench class="h-5 w-5" />
  if (id === 'notes') return <BookOpen class="h-5 w-5" />
  return <Wrench class="h-5 w-5" />
}

function formatDataTime(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date)
}

export default function WorkspacePage() {
  const [craftData] = createResource(loadCraftData)

  return (
    <div class="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6 lg:px-8">
      <div class="flex flex-col gap-2">
        <div class="text-sm text-muted-foreground">工作台</div>
        <h1 class="text-2xl font-semibold">XIV Companion</h1>
      </div>

      <section class="rounded-md border bg-muted/30 px-3 py-2.5">
        <Show
          when={craftData()}
          fallback={<div class="h-8 rounded bg-muted" />}
        >
          {(data) => (
            <div class="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
              <div class="flex items-center gap-2 font-medium">
                <Database class="h-4 w-4 text-muted-foreground" />
                数据
              </div>
              <div class="min-w-0">
                <span class="text-muted-foreground">游戏版本 </span>
                <span class="font-medium">{data().gameVersion}</span>
              </div>
              <div>
                <span class="text-muted-foreground">生成 </span>
                <span class="font-medium">{formatDataTime(data().generatedAt)}</span>
              </div>
              <div>
                <span class="text-muted-foreground">物品 </span>
                <span class="font-medium">{formatInteger(data().counts.items)}</span>
              </div>
              <div>
                <span class="text-muted-foreground">配方 </span>
                <span class="font-medium">{formatInteger(data().counts.recipes)}</span>
              </div>
              <div>
                <span class="text-muted-foreground">来源 </span>
                <span class="font-medium">{formatInteger(data().counts.sources)}</span>
              </div>
            </div>
          )}
        </Show>
      </section>

      <section class="space-y-3">
        <div>
          <div class="text-sm font-medium">工具</div>
          <div class="mt-1 text-sm text-muted-foreground">当前可用的工作区</div>
        </div>

        <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <For each={appModules}>
            {(module) => {
              return (
                <A href={module.href} class="block">
                  <Card class={cx('h-full transition-colors hover:border-foreground/20')}>
                    <CardHeader>
                      <div class="flex h-10 w-10 items-center justify-center rounded-lg border bg-background text-muted-foreground">
                        {iconFor(module.id)}
                      </div>
                    </CardHeader>
                    <CardContent class="space-y-2">
                      <CardTitle>{module.label}</CardTitle>
                      <div class="text-sm text-muted-foreground">
                        {module.id === 'character' && '职业等级、任务进度、生产三围'}
                        {module.id === 'crafting' && '配方树、素材汇总、来源选择'}
                        {module.id === 'notes' && '目录页面、分栏卡片、材料汇总'}
                      </div>
                    </CardContent>
                  </Card>
                </A>
              )
            }}
          </For>
        </div>
      </section>
    </div>
  )
}
