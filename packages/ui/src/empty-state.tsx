import type { JSX } from 'solid-js'

export function EmptyState(props: {
  icon?: JSX.Element
  title: string
  description?: string
  action?: JSX.Element
}) {
  return (
    <div class="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-dashed bg-background p-6 text-center">
      {props.icon && <div class="text-muted-foreground">{props.icon}</div>}
      <div class="text-sm font-medium">{props.title}</div>
      {props.description && <div class="max-w-sm text-sm text-muted-foreground">{props.description}</div>}
      {props.action && <div class="pt-2">{props.action}</div>}
    </div>
  )
}

