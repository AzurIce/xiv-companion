import type { JSX } from 'solid-js'
import { splitProps } from 'solid-js'
import { cx } from '@xiv-companian/shared'

export interface BadgeProps extends JSX.HTMLAttributes<HTMLSpanElement> {
  variant?: 'default' | 'secondary' | 'outline' | 'success' | 'warning'
}

export function Badge(props: BadgeProps) {
  const [local, rest] = splitProps(props, ['class', 'variant'])
  const variant = () => local.variant ?? 'secondary'

  return (
    <span
      class={cx(
        'inline-flex h-5 items-center rounded px-1.5 text-xs font-medium',
        variant() === 'default' && 'bg-primary text-primary-foreground',
        variant() === 'secondary' && 'bg-secondary text-secondary-foreground',
        variant() === 'outline' && 'border border-border bg-background',
        variant() === 'success' && 'border border-emerald-200 bg-emerald-50 text-emerald-700',
        variant() === 'warning' && 'border border-amber-200 bg-amber-50 text-amber-700',
        local.class,
      )}
      {...rest}
    />
  )
}

