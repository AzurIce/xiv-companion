import { appModules } from '@xiv-companian/shared'
import { Card, CardContent, CardHeader, CardTitle } from '@xiv-companian/ui'

export default function PlaceholderPage(props: { moduleId: string }) {
  const module = () => appModules.find((item) => item.id === props.moduleId)
  const title = () => module()?.label ?? '设置'

  return (
    <div class="mx-auto flex max-w-7xl flex-col gap-4 px-4 py-6 sm:px-6 lg:px-8">
      <div>
        <div class="text-sm text-muted-foreground">模块</div>
        <h1 class="text-2xl font-semibold">{title()}</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{title()}</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="text-sm text-muted-foreground">工作区尚未接入。</div>
        </CardContent>
      </Card>
    </div>
  )
}

