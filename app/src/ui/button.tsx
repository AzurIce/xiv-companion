import type { JSX } from 'solid-js'
import { splitProps } from 'solid-js'
import { cx } from '../lib'

export interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'outline' | 'ghost'
  size?: 'sm' | 'md' | 'icon'
}

export function Button(props: ButtonProps) {
  const [local, rest] = splitProps(props, ['class', 'variant', 'size'])
  const variant = () => local.variant ?? 'secondary'
  const size = () => local.size ?? 'md'

  return (
    <button
      class={cx(
        'inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium transition-colors',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
        'disabled:pointer-events-none disabled:opacity-50',
        variant() === 'primary' && 'bg-primary text-primary-foreground hover:bg-primary/90',
        variant() === 'secondary' && 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
        variant() === 'outline' && 'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
        variant() === 'ghost' && 'hover:bg-accent hover:text-accent-foreground',
        size() === 'sm' && 'h-8 px-3',
        size() === 'md' && 'h-9 px-3',
        size() === 'icon' && 'h-8 w-8',
        local.class,
      )}
      {...rest}
    />
  )
}
