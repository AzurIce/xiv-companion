import { HashRouter, Route } from '@solidjs/router'
import type { JSX } from 'solid-js'
import AppShell from './layout/AppShell'
import CraftingPage from './modules/crafting/CraftingPage'
import CharacterPage from './pages/CharacterPage'
import NotesPage from './pages/NotesPage'
import WorkspacePage from './pages/WorkspacePage'

function Layout(props: { children?: JSX.Element }) {
  return <AppShell>{props.children}</AppShell>
}

export default function App() {
  return (
    <HashRouter root={Layout}>
      <Route path="/" component={WorkspacePage} />
      <Route path="/character" component={CharacterPage} />
      <Route path="/crafting" component={CraftingPage} />
      <Route path="/notes" component={NotesPage} />
    </HashRouter>
  )
}
