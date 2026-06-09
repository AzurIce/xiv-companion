import type { JSX } from 'solid-js'
import { splitProps } from 'solid-js'
import { cx } from '@xiv-companian/shared'

export function Separator(props: JSX.HTMLAttributes<HTMLDivElement>) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx('h-px w-full bg-border', local.class)} {...rest} />
}

