export type ClassValue = string | false | null | undefined

export function cx(...values: ClassValue[]): string {
  return values.filter(Boolean).join(' ')
}

export function formatInteger(value: number): string {
  return Number.isFinite(value) ? value.toLocaleString('zh-CN') : '-'
}

export function baseUrl(): string {
  return import.meta.env.BASE_URL || '/'
}

