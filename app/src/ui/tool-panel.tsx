import type { JSX } from 'solid-js'
import { splitProps } from 'solid-js'
import { cx } from '../lib'

export function ToolPanel(props: JSX.HTMLAttributes<HTMLDivElement>) {
  const [local, rest] = splitProps(props, ['class'])
  return (
    <div
      class={cx('rounded-lg border bg-card text-card-foreground shadow-sm', local.class)}
      {...rest}
    />
  )
}
