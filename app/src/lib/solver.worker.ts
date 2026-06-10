import initWasm, { solveRaphaelMacro } from '../wasm/xiv_companion'
import type {
  CraftRecipe,
  CrafterAttributes,
  MacroSolveResult,
  RaphaelSolveOptions,
  RecipeLevelInfo,
} from '../wasm/xiv_companion'

type SolveMessage = {
  recipe: CraftRecipe
  recipeLevel: RecipeLevelInfo
  attrs: CrafterAttributes
  options: RaphaelSolveOptions
}

self.onmessage = async (event: MessageEvent<SolveMessage>) => {
  try {
    await initWasm()
    const { recipe, recipeLevel, attrs, options } = event.data
    const result: MacroSolveResult = solveRaphaelMacro(recipe, recipeLevel, attrs, options)
    self.postMessage({ result })
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : String(error) })
  }
}
