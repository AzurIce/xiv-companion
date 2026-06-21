# Raphael RCSP Bound Lab

This lab prototype compares:

- baseline Raphael `MacroSolver`
- Raphael seeded with an external incumbent
- a simplified incumbent-bound relaxed suffix/PDB proof

It intentionally lives outside the root workspace and does not modify app code.

## Upstream Patch

The seeded Raphael experiment needs an experimental API added to the ignored upstream checkout:

```bash
git -C ../upstream/raphael-rs apply ../raphael-rcsp-bound/raphael-solver-initial-solution.patch
```

The patch adds:

```rust
MacroSolver::solve_with_initial_solution(initial_actions)
```

The current local checkout already has this patch applied.

## Run

```bash
cargo run --manifest-path lab/raphael-rcsp-bound/Cargo.toml --release
```

The output includes Raphael baseline/seeded stats, the generated incumbent, and a budget table for the relaxed suffix proof.
