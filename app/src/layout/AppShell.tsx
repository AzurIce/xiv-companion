import type { JSX } from 'solid-js'
import { createEffect, createSignal, For } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import {
  BookOpen,
  UserRoundCog,
  Home,
  LayoutDashboard,
  PanelLeftClose,
  PanelLeftOpen,
  Wrench,
} from 'lucide-solid'
import { appModules, moduleGroupLabel, type AppModule } from '../lib'

const groups = Array.from(new Set(appModules.map((module) => module.group))) as AppModule['group'][]

function ModuleIcon(props: { id: string; class?: string }) {
  const iconClass = () => props.class ?? 'h-4 w-4'
  if (props.id === 'character') return <UserRoundCog class={iconClass()} />
  if (props.id === 'crafting') return <Wrench class={iconClass()} />
  if (props.id === 'notes') return <BookOpen class={iconClass()} />
  return <Wrench class={iconClass()} />
}

function IconTooltip(props: { label: string; enabled?: boolean; class?: string; children: JSX.Element }) {
  return (
    <div class={`group ${props.class ?? 'relative'}`}>
      {props.children}
      {props.enabled && (
        <div class="pointer-events-none absolute left-full top-1/2 z-50 ml-2 hidden -translate-y-1/2 whitespace-nowrap rounded-md border bg-popover px-2 py-1 text-xs text-popover-foreground shadow-md group-hover:block">
          {props.label}
        </div>
      )}
    </div>
  )
}

function ModuleLink(props: { module: AppModule; compact?: boolean; collapsed?: boolean }) {
  const link = (
    <A
      href={props.module.href}
      class="flex items-center rounded-md text-sm font-medium text-muted-foreground transition-all duration-300 ease-out"
      classList={{
        'h-10 min-w-36 gap-3 px-3': !!props.compact,
        'h-9 gap-3 px-3': !props.compact && !props.collapsed,
        'h-10 justify-center px-0': !props.compact && !!props.collapsed,
      }}
      activeClass="bg-accent text-foreground"
      title={props.collapsed ? props.module.label : undefined}
    >
      <ModuleIcon id={props.module.id} />
      <span
        class="min-w-0 truncate whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out"
        classList={{
          'max-w-0 -translate-x-1 opacity-0': !!props.collapsed,
          'max-w-40 translate-x-0 opacity-100': !props.collapsed,
        }}
      >
        {props.module.label}
      </span>
    </A>
  )

  if (props.compact) return link
  return <IconTooltip label={props.module.label} enabled={props.collapsed}>{link}</IconTooltip>
}

export default function AppShell(props: { children?: JSX.Element }) {
  const location = useLocation()
  const [collapsed, setCollapsed] = createSignal(localStorage.getItem('xiv-companion-sidebar') === 'collapsed')
  const activeModule = () => appModules.find((module) => location.pathname.startsWith(module.href))

  createEffect(() => {
    localStorage.setItem('xiv-companion-sidebar', collapsed() ? 'collapsed' : 'expanded')
  })

  return (
    <div
      class="min-h-screen bg-background text-foreground lg:grid lg:transition-[grid-template-columns] lg:duration-300 lg:ease-out"
      style={`grid-template-columns: ${collapsed() ? '72px minmax(0,1fr)' : '260px minmax(0,1fr)'}`}
    >
      <aside class="hidden min-h-screen min-w-0 overflow-visible border-r bg-card transition-all duration-300 ease-out lg:flex lg:flex-col">
        <div
          class="relative flex h-16 items-center border-b transition-all duration-300 ease-out"
          classList={{
            'justify-center px-0': collapsed(),
            'gap-3 px-3 pr-8': !collapsed(),
          }}
        >
          <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <LayoutDashboard class="h-4 w-4" />
          </div>
          <div
            class="min-w-0 overflow-hidden whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out"
            classList={{
              'max-w-0 -translate-x-1 opacity-0': collapsed(),
              'max-w-40 translate-x-0 opacity-100': !collapsed(),
            }}
          >
            <div class="text-sm font-semibold">XIV Companion</div>
            <div class="text-xs text-muted-foreground">Eorzea workspace</div>
          </div>
          <IconTooltip
            label={collapsed() ? '展开侧边栏' : '折叠侧边栏'}
            enabled
            class="absolute -right-3 top-1/2 z-20 -translate-y-1/2"
          >
            <button
              type="button"
              class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full border bg-card text-muted-foreground shadow-sm transition-colors duration-200 hover:border-foreground/20 hover:bg-accent hover:text-foreground"
              onClick={() => setCollapsed(!collapsed())}
              aria-label={collapsed() ? '展开侧边栏' : '折叠侧边栏'}
              title={collapsed() ? '展开侧边栏' : '折叠侧边栏'}
            >
              {collapsed() ? <PanelLeftOpen class="h-3.5 w-3.5" /> : <PanelLeftClose class="h-3.5 w-3.5" />}
            </button>
          </IconTooltip>
        </div>

        <div class="flex-1 overflow-y-auto px-3 py-4">
          <IconTooltip label="工作台" enabled={collapsed()}>
            <A
              href="/"
              end
              class="mb-4 flex rounded-md text-sm font-medium text-muted-foreground transition-all duration-300 ease-out"
              classList={{
                'h-10 items-center justify-center': collapsed(),
                'h-9 items-center gap-3 px-3': !collapsed(),
              }}
              activeClass="bg-accent text-foreground"
              title={collapsed() ? '工作台' : undefined}
            >
              <Home class="h-4 w-4" />
              <span
                class="overflow-hidden whitespace-nowrap transition-[max-width,opacity,transform] duration-300 ease-out"
                classList={{
                  'max-w-0 -translate-x-1 opacity-0': collapsed(),
                  'max-w-28 translate-x-0 opacity-100': !collapsed(),
                }}
              >
                工作台
              </span>
            </A>
          </IconTooltip>

          <For each={groups}>
            {(group) => {
              const modules = appModules.filter((module) => module.group === group)
              return (
                <section class="mb-5">
                  {!collapsed() && (
                    <div class="mb-2 px-3 text-xs font-medium text-muted-foreground">
                      {moduleGroupLabel(group)}
                    </div>
                  )}
                  <nav class="space-y-1" aria-label={moduleGroupLabel(group)}>
                    <For each={modules}>
                      {(module) => <ModuleLink module={module} collapsed={collapsed()} />}
                    </For>
                  </nav>
                </section>
              )
            }}
          </For>
        </div>

      </aside>

      <div class="flex min-w-0 flex-col">
        <header class="sticky top-0 z-40 border-b bg-background/95 backdrop-blur lg:hidden">
          <div class="flex h-14 items-center gap-3 px-4">
            <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <LayoutDashboard class="h-4 w-4" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-sm font-semibold">XIV Companion</div>
              <div class="truncate text-xs text-muted-foreground">
                {activeModule()?.label ?? '工作台'}
              </div>
            </div>
          </div>
          <nav class="flex gap-2 overflow-x-auto px-4 pb-3" aria-label="模块">
            <A
              href="/"
              end
              class="flex h-10 min-w-28 items-center gap-2 rounded-md border bg-card px-3 text-sm font-medium text-muted-foreground"
              activeClass="text-foreground border-foreground/20"
            >
              <Home class="h-4 w-4" />
              工作台
            </A>
            <For each={appModules}>
              {(module) => (
                <div class="rounded-md border bg-card">
                  <ModuleLink module={module} compact />
                </div>
              )}
            </For>
          </nav>
        </header>

        <main class="min-w-0 flex-1">{props.children}</main>
      </div>
    </div>
  )
}
