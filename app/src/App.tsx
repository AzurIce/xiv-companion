import { HashRouter, Route } from '@solidjs/router'
import type { JSX } from 'solid-js'
import AppShell from './layout/AppShell'
import CraftingPage from './modules/crafting/CraftingPage'
import WorkspacePage from './pages/WorkspacePage'
import PlaceholderPage from './pages/PlaceholderPage'

function Layout(props: { children?: JSX.Element }) {
  return <AppShell>{props.children}</AppShell>
}

export default function App() {
  return (
    <HashRouter root={Layout}>
      <Route path="/" component={WorkspacePage} />
      <Route path="/crafting" component={CraftingPage} />
      <Route path="/glamour" component={() => <PlaceholderPage moduleId="glamour" />} />
      <Route path="/housing" component={() => <PlaceholderPage moduleId="housing" />} />
      <Route path="/library" component={() => <PlaceholderPage moduleId="library" />} />
      <Route path="/settings" component={() => <PlaceholderPage moduleId="settings" />} />
    </HashRouter>
  )
}
