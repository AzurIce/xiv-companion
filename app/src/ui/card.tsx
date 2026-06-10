import type { JSX } from 'solid-js'
import { splitProps } from 'solid-js'
import { cx } from '../lib'

export interface CardProps extends JSX.HTMLAttributes<HTMLDivElement> {}

export function Card(props: CardProps) {
  const [local, rest] = splitProps(props, ['class'])
  return (
    <div
      class={cx('rounded-lg border bg-card text-card-foreground shadow-sm', local.class)}
      {...rest}
    />
  )
}

export function CardHeader(props: CardProps) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx('space-y-1 p-4 pb-2', local.class)} {...rest} />
}

export function CardTitle(props: CardProps) {
  const [local, rest] = splitProps(props, ['class'])
  return <h2 class={cx('text-base font-semibold', local.class)} {...rest} />
}

export function CardContent(props: CardProps) {
  const [local, rest] = splitProps(props, ['class'])
  return <div class={cx('p-4 pt-2', local.class)} {...rest} />
}
