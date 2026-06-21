# 制作宏求解算法调研

本文记录对 `KonaeAkira/raphael-rs` 与 `Tnze/ffxiv-best-craft` 的实现阅读、验证和结论。调研目标：

1. 是否有更加高效的算法。
2. 等级同步配方是否能支持指定等级区间，并限制求解出的宏在区间内全部可用。
3. 能否从目标状态逆向求解装备的最低要求。

## 调研快照

- 本地项目当前依赖 `raphael-rs` 的 `v0.28.4`，tag commit 为 `cdb6256c752a2230daffbfdcb7377e32937dcf4d`。
- `raphael-rs` 当前上游 `origin/HEAD` 指向 `preview` 分支，commit 为 `1563ada6a1ac9c54ec861aabb866c50d89222069`。相对 `v0.28.4`，`raphael-sim` 与 `raphael-solver` 没有变更，差异集中在 UI 重构。
- `ffxiv-best-craft` 当前上游 HEAD 为 `0b37d33c30fdb6b253a5559bc1b8cbd783c91b8c`。
- `ffxiv-best-craft/src-libs` 当前依赖 `ffxiv-crafting v7.4.5`，并同样依赖 `raphael-rs v0.28.4` 的 `raphael-sim` 与 `raphael-solver`。

本次实验放在 `lab/upstream`：

- `lab/upstream/raphael-rs`
- `lab/upstream/ffxiv-best-craft`

验证命令：

- `cargo test -p raphael-sim --test action_tests`，35 个测试通过。
- `cargo test -p raphael-solver --test 00_edge_cases`，11 个测试通过。高 `stellar_steady_hand_charges` 边界用例耗时约 85 秒。
- `cargo check -p app-libs`，`ffxiv-best-craft/src-libs` 通过检查。

本项目已有 `src/game_data.rs` 未提交改动，本次调研未修改该文件。

## 本项目当前接入点

本项目的 `src/solver.rs` 已经通过 `raphael-sim` 与 `raphael-solver` 接入了 Raphael 宏求解：

- 根据 `CraftRecipe` 与 `RecipeLevelInfo` 计算 `max_progress`、`max_quality`、`max_durability`。
- 根据玩家属性计算 `base_progress` 与 `base_quality`。
- 用 `RaphaelSolveOptions` 控制 `Manipulation`、`Heart and Soul`、`Quick Innovation`、`Trained Eye`、`backload_progress`、`adversarial`、`Stellar Steady Hand` 等选项。
- 构造 `raphael_simulator::Settings` 后调用 `MacroSolver::solve()`。

当前还没有把 `GathererCrafterLvAdjustTable` 导出到前端数据包。虽然 `CraftRecipe` 已有 `max_level_scaling` 字段，但缺少按同步等级映射 `RecipeLevelTable` 的表，因此无法完整复刻 Raphael 对等级同步配方的处理。

## Raphael 的实现

Raphael 的核心在 `raphael-solver` 和 `raphael-sim`。

### 模拟器

`raphael-sim` 的 `Settings` 包含：

- `max_cp`
- `max_durability`
- `max_progress`
- `max_quality`
- `base_progress`
- `base_quality`
- `job_level`
- `allowed_actions`
- `adversarial`
- `backload_progress`
- `stellar_steady_hand_charges`

技能是否可用由两部分决定：

- `job_level >= ActionImpl::LEVEL_REQUIREMENT`
- `allowed_actions` 包含该技能所需 mask

除了解锁等级外，`job_level` 还影响若干技能效果：

| 规则 | 影响 |
| --- | --- |
| `job_level >= 11` | 品质技能开始增加 Inner Quiet |
| `Basic Synthesis` 31 级特性 | 作业效率从 100 提升到 120 |
| `Rapid Synthesis` 63 级特性 | 作业效率从 250 提升到 500 |
| `Careful Synthesis` 82 级特性 | 作业效率从 150 提升到 180 |
| `Groundwork` 86 级特性 | 作业效率从 300 提升到 360 |
| `Delicate Synthesis` 94 级特性 | 作业效率从 100 提升到 150 |
| `Hasty Touch` 96 级后 | 在 Expedience 相关状态下可能变为 `Daring Touch` 路径 |

这意味着“等级区间保证可用”不能只看技能解锁表。若目标是区间内保证成功，也必须考虑这些等级特性导致的状态变化。

### 宏求解器

`MacroSolver` 不是朴素 DFS/BFS。整体流程是：

1. 构造初始 `SimulationState`。
2. `FinishSolver::precompute()` 预计算“从任意中间状态是否还能完成进展”。
3. 用 `FinishSolver::can_finish(initial_state)` 早期判断无解。
4. `QualityUbSolver::precompute()` 预计算质量上界。
5. 如果初始状态质量上界足以达到目标质量，再预计算 `StepLbSolver`。
6. 主搜索按 `SearchScore` 弹出一批同分节点并并行展开。
7. 每个候选状态先用 `FinishSolver` 剪掉不能完成进展的分支。
8. 再用 `QualityUbSolver` 计算质量上界，质量上界不足时剪枝。
9. 对可能满品质的状态，用 `StepLbSolver` 估计至少还需要多少步。
10. `SearchQueue` 用 Pareto 前沿去除支配状态。

`SearchScore` 优先级大致是：

- 质量上界越高越优先。
- 最小步数下界越小越优先。
- 最小耗时下界越小越优先。
- 当前步数与当前耗时越小越优先。

Raphael 还把常见固定组合合并为 `ActionCombo`，例如：

- `Basic Touch + Standard Touch`
- `Basic Touch + Standard Touch + Advanced Touch`
- `Basic Touch + Refined Touch`
- `Observe + Advanced Touch`
- `Heart and Soul + Tricks of the Trade`
- `Heart and Soul + Intensive Synthesis`
- `Heart and Soul + Precise Touch`

这减少了搜索分支，也避免在状态空间里保留许多等价 combo 状态。

### Pareto 剪枝

`SearchQueue` 在弹出 batch 时重放父节点动作得到当前状态，然后用 Pareto 前沿过滤。

Raphael 的支配关系不是简单比较完整状态，而是把状态拆成：

- key：进展值，以及不参与“数值大小比较”的效果位。
- value：CP、耐久、品质、可靠品质上界、部分效果计数。

在同一 key 下，如果状态 A 在 CP、耐久、品质、效果计数等维度都不低于状态 B，A 支配 B，B 可以删除。

实现上用 SIMD 做支配比较，并按 CP 分裂叶子，避免单个 bucket 线性增长过大。

### FinishSolver

`FinishSolver` 只关心能否补完进展。它把状态压缩成：

- `durability`
- 去掉品质相关效果后的 `Effects`
- CP 到最大可追加进展的 breakpoints

预计算后，查询 `can_finish(state)` 很快。如果某个状态即使把剩余资源都用于进展也完不成，该分支直接剪掉。

### QualityUbSolver

`QualityUbSolver` 给出从当前状态出发能达到的质量上界。为了让上界容易计算，它做了几类放松：

- 把当前耐久、Manipulation、Trained Perfection 等折算成 CP。
- 把状态耐久规整到一个高值。
- 压缩不可靠品质。
- 对 Muscle Memory 按最大利用估算。
- 用 Pareto 前沿维护“追加进展、追加品质”的上界集合。

这不是为了构造真实宏，而是为了证明“这条分支最多也做不到目标质量”，因此可以剪枝。

### StepLbSolver

`StepLbSolver` 用于估计达到目标质量至少需要几步。它进一步放松状态：

- 忽略 CP 成本。
- 让 `Trained Perfection`、`Quick Innovation` 等有限次数能力近似为始终可用。
- 把 `Great Strides`、`Waste Not` 等效果简化为较少状态。
- 对 `Innovation` 与 `Veneration` 做状态合并。

这个下界越紧，越能帮助主搜索优先找到短宏并丢弃低分候选。

## ffxiv-best-craft 的实现

`ffxiv-best-craft` 有多套 solver。

### Raphael 适配层

`src-libs/src/solver/raphael.rs` 是对 `raphael-rs v0.28.4` 的封装。它把 `ffxiv-crafting::Status` 转成 Raphael 的 `Settings`，然后调用 `MacroSolver`。

这个适配层与本项目当前 `src/solver.rs` 很接近。

### Depth-first search solver

`depth_first_search_solver.rs` 是深度限制 DFS：

- 对固定技能列表逐个尝试。
- 非 wasm 环境下会按可用 CPU 开线程分支搜索。
- 分数比较优先级为进展、品质、步数。
- 剪枝主要依赖最大深度、技能可用性、已获得满品质后的步数约束。

它实现简单，适合小深度或低等级配方，但面对完整高等级技能集合时状态爆炸明显。

### Normal progress solver

`normal_progress_solver.rs` 是只求完成进展的递归搜索：

- 技能列表只含推进进展、恢复耐久、观察等动作。
- 返回能完成进展的最短动作序列。

它比完整 DFS 小很多，但只解决 NQ 或收尾进展问题。

### Reflect DP solver

`reflect_solver.rs` 是 `ffxiv-best-craft` 更有价值的自研 DP：

- `ProgressSolver` 用状态数组缓存“剩余资源最多还能推多少进展”。
- `QualitySolver` 在保证 `ProgressSolver` 可收尾的前提下最大化品质。
- 状态维度包括 combo、Inner Quiet、Innovation、Great Strides、Manipulation、Waste Not、Trained Perfection、耐久、CP 等。
- `read_all` 逐步读出当前状态下最优下一步，生成整条宏。
- 它会比较普通开局与 `Reflect` 开局。

这套 DP 对固定策略族很快，但技能覆盖不完整，例如 `Quick Innovation` 被注释，`Heart and Soul`、随机/专家条件、对抗模式等也不是完整 Raphael 问题。它更像一个快速专用 solver，而不是完整替代。

### 适用范围分析

`src-libs/src/analyzer/scope_of_application.rs` 已有“装备属性适配范围”能力：

- 对 craftsmanship 从当前值向下线性扫描，找仍能完成进展的最低值。
- 对 craftsmanship 向上扫描，找因过早完成导致步数变化的上界。
- 对 control 从当前值向下线性扫描，找仍能满品质的最低值。
- CP 直接返回初始 CP 与最终 CP 差。

这是逆向装备要求的可行原型，但目前是线性枚举，并且只覆盖固定宏。

## 问题 1：是否有更加高效的算法

结论需要分层表述：对“完整动作集合、确定性普通条件、尽量求短/高质量宏”的通用问题，Raphael 已经是这两个项目中最强的现成实现。没有发现 `ffxiv-best-craft` 中存在一个能直接替代 Raphael 且更完整、更快的算法。

这不等于理论上没可能更高效。宏求解本质上是组合搜索，若要求完整性、稳定无解证明、较优步数和完整技能语义，最坏情况仍会指数增长。但如果愿意改变目标、强化启发式或利用 FFXIV 宏的结构特征，仍有明确的提速空间。

### 可直接复用的更快专用算法

BestCraft 的 `ReflectSolver` 是一个可作为“快速候选宏生成器”的 DP：

- 优点：状态数组直接索引，生成速度通常会比完整搜索快。
- 缺点：动作集合、模式和条件覆盖不完整，不保证全局最优，也不适合所有配方。

建议用途不是替代 Raphael，而是：

1. 先用专用 DP 生成一个候选宏。
2. 用 Raphael 模拟器验证候选宏。
3. 如果候选宏满足目标，可以直接返回，或者作为 Raphael 的初始 incumbent 来提高剪枝。

当前 Raphael API 没有暴露“外部初始解”。如果想让这条路径真正提速，需要 fork 或扩展 `MacroSolver`，让调用方传入一条已验证候选宏，并据此初始化 `min_accepted_score`。

### 真正可能更快的研究方向

这些方向都有机会比 Raphael 快，但通常会牺牲一部分通用性，或需要比较大的 solver 改造。

1. 结构化两阶段 DP

   大多数实用宏可以近似拆成“品质阶段 + 收尾进展阶段”，尤其开启 `backload_progress` 时更明显。可以像 BestCraft 的 `ReflectSolver` 那样先用 DP 求品质阶段，同时用 progress DP 判断是否可收尾。这个方向可能非常快，但会漏掉“中途穿插进展以触发/保留资源优势”的宏。

2. 带外部 incumbent 的 A*/branch-and-bound

   Raphael 已经是 branch-and-bound 风格，但初始时没有外部候选解。若先用快速 DP、模板库、历史宏或贪心策略给出一条可行宏，主搜索可以更早提高接受分数，剪掉大量低潜力分支。这个方向基本不牺牲正确性，是最值得优先尝试的优化。

3. 更强的 admissible heuristic / pattern database

   Raphael 的质量上界与步数下界已经很强，但还可以研究更细的模式库，例如只针对“高等级常用 buff 状态 + 耐久/CP bucket”预计算可达上界。只要 heuristic 仍是上界/下界，就不影响正确性。代价是预计算与内存。

4. 反向或双向搜索

   从目标状态反推可行前驱在理论上能缩短搜索深度，但制作状态里有 CP 消耗、耐久恢复、buff tick、combo、一次性技能和提前完成，反向转移会一对多且约束复杂。它更适合局部使用，例如 progress finish 或固定宏逆向属性，而不一定适合完整宏求解。

5. 多目标 DP 替代全局搜索

   对给定动作集合，可以维护 `(progress, quality, cp, durability, effects)` 的 Pareto 前沿，按步数逐层扩展。这在状态压缩足够强时很快，也容易给出最短步数证明。但完整 effects 维度很大，若压缩过强会变成近似 solver，若压缩不足会爆内存。Raphael 的 ParetoFront 已经吸收了这条路线的一部分。

6. 统计/学习型排序

   用历史宏、配方类别、等级段训练动作排序或策略网络，可以更快找到可行解。它适合“先给用户一个好宏”，但不能单独提供无解证明。较稳妥的做法是只把它当动作排序或 incumbent 生成器，仍由 Raphael 验证。

7. 模板化宏搜索

   许多宏符合少量 skeleton，例如 `Reflect -> IQ 构建 -> Byregot -> progress finish`。先枚举模板参数，再调用模拟器验证，会非常快。缺点是模板外的解会被漏掉。适合做“快速模式”，不适合替代“完整模式”。

### 更像“超越 Raphael”的候选架构

如果目标是**在常见宏实例上明显快于 Raphael，同时保留完整性**，我现在更看好的不是单一 DP，而是一个组合式框架：

```text
Fast incumbent  ->  BFWS / novelty search  ->  PDB / CEGAR heuristic refinement  ->  exact label search
```

它的核心想法是：

1. **先快给一个可行解**  
   用模板化宏、专用 DP、甚至贪心策略迅速得到一条能过的宏。  
   这一步是为了把后续精确搜索的上界压低。资源约束最短路文献一再强调，强初始解能显著帮助剪枝。

2. **再用宽度/novelty 搜索突破平台期**  
   Raphael 的批处理搜索会在很多相似状态上花时间。  
   BFWS 类方法用 `novelty` 区分“是否出现过新的特征组合”，能在启发式失效的平坦区域里更有效地探索。  
   对制作宏来说，特征可以是：
   - progress / quality bucket
   - CP / durability bucket
   - buff 组合
   - combo / opener 状态
   - 是否已经进入 progress finish

3. **用 PDB 给出更强下界**  
   对抽象状态预计算 pattern database，可以得到比单纯线性估计更紧的剩余步数或剩余资源下界。  
   这里的关键不是把全部状态都精确建模，而是只建“对宏有影响”的局部模式，例如：
   - Inner Quiet + Great Strides + Innovation + Veneration
   - durability / CP bucket
   - 进展是否已经跨过某个门槛

4. **用 CEGAR 只在真的需要时细化**  
   如果抽象计划在具体状态上失败，就把导致失败的特征拆细。  
   这比一开始就把全部 buff 维度展开更省。  
   对等级区间问题，CEGAR 也很自然：先用端点等级或少数 witness levels 搜索，失败后再加入反例等级。

5. **把宏问题看成带资源约束的最短路/多目标最短路**  
   CP、耐久、步数、耗时都很像 resource constrained shortest path。  
   如果要同时保留多种目标，不该只用单一标量分数，而应使用 Pareto label-setting 或 NAMOA* 风格的标签集。  
   这样可以更自然地保留“步数更短但品质稍低”与“品质更高但耗时更长”的并行候选。

我认为这条线比“继续把单体 solver 做得更大”更有希望。  
原因是它把几个互补的强项拼在一起了：

- 快速 incumbent 负责把上界压低。
- BFWS 负责摆脱启发式平台。
- PDB / CEGAR 负责让 heuristic 真正跟上域结构。
- label-setting / Pareto 负责完整性和多目标保留。

### 相关原始文献

- Bylander, *The Computational Complexity of Propositional STRIPS Planning*  
  [PDF](https://ai.dmi.unibas.ch/research/reading_group/bylander-aij1994.pdf)
- Width and Serialization of Classical Planning Problems  
  [PDF](https://www-i6.informatik.rwth-aachen.de/~hector.geffner/www.dtic.upf.edu/~hgeffner/width-ecai-2012.pdf)
- Best-First Width Search: Exploration and Exploitation in Classical Planning  
  [AAAI](https://ojs.aaai.org/index.php/AAAI/article/view/11027)
- Pattern Databases  
  [PDF](https://webdocs.cs.ualberta.ca/~jonathan/publications/ai_publications/compi.pdf)
- Counterexample-Guided Cartesian Abstraction Refinement  
  [JAIR](https://jair.org/index.php/jair/article/view/11217)
- A Fast Exact Algorithm for the Resource Constrained Shortest Path Problem  
  [PDF](https://cdn.aaai.org/ojs/17450/17450-13-20944-1-2-20210518.pdf)
- An Exact Bidirectional A* Approach for Solving Resource-Constrained Shortest Path Problems  
  [Networks](https://onlinelibrary.wiley.com/doi/10.1002/net.21856)
- Resource-Constrained Pathfinding with Enhanced Bidirectional A* Search  
  [AAAI PDF](https://ojs.aaai.org/index.php/AAAI/article/view/34892/37047)
- An Exact Bidirectional Pulse Algorithm for the Constrained Shortest Path  
  [Networks](https://onlinelibrary.wiley.com/doi/abs/10.1002/net.21960)
- Heuristic-Search Approaches for the Multi-Objective Shortest-Path Problem  
  [PDF](https://www.ijcai.org/proceedings/2023/0757.pdf)
- EMOA*: A framework for search-based multi-objective path planning  
  [PDF](https://biorobotics.ri.cmu.edu/papers/paperUploads/1-s2.0-S0004370224001966-main.pdf)
- A Simple and Fast Bi-Objective Search Algorithm  
  [PDF](https://yeoh-lab.wustl.edu/assets/pdf/icaps-Ulloa0BZSK20.pdf)
- ARA*: Anytime A* with Provable Bounds on Sub-Optimality  
  [NeurIPS PDF](https://papers.neurips.cc/paper/2382-ara-anytime-a-with-provable-bounds-on-sub-optimality.pdf)
- Anytime Heuristic Search  
  [JAIR PDF](https://jair.org/index.php/jair/article/download/10489/25135/19484)
- Potential-based Bounded-Cost Search and Anytime Non-Parametric A*  
  [PDF](https://goldberg.berkeley.edu/pubs/Anytime-Nonparametric-A-star-AI-Journal-Sept-2014.pdf)
- Solving Domain-Independent Dynamic Programming Problems with Anytime Heuristic Search  
  [PDF](https://tidel.mie.utoronto.ca/pubs/Solving%20Domain%20Independent%20Dynamic%20Programming%20Problems%20with%20Anytime%20Heuristic%20Search.pdf)

### 候选算法规格

我现在认为最值得继续推的版本可以写成下面这样：

```text
1. fast incumbent generator
2. BFWS frontier search over abstract macro states
3. multi-label Pareto pruning
4. PDB heuristic for remaining quality/progress/resources
5. CEGAR refinement for failed abstractions and level witnesses
6. exact verification on concrete states
```

#### 状态表示

不要一开始就把完整 `SimulationState` 当作唯一搜索粒度，而是拆成两层：

- `concrete state`：用于最终验证和 action replay。
- `abstract state`：用于搜索和剪枝。

推荐的抽象特征：

- progress bucket
- quality bucket
- CP bucket
- durability bucket
- Inner Quiet bucket
- Great Strides / Innovation / Veneration / Waste Not / Manipulation / Heart and Soul / Quick Innovation / Stellar Steady Hand 的 presence-or-bucket
- combo / opener type
- 是否已经进入收尾阶段
- 是否还允许 quality action

这些特征大多和技能转移的“结构”有关，而不需要每次都保留完整数值。  
这正是 width-based planning 最擅长的地方。

#### 启发式

用三类启发式同时驱动：

1. `h_pdb`
   - 从抽象状态到目标所需的最少剩余步数/资源。
   - 来自 pattern database。

2. `novelty`
   - 某个新状态是否引入了新的特征组合。
   - 用来打破启发式平台。

3. `rcsp_bound`
   - 把宏问题视作资源约束最短路。
   - 对 CP、耐久、步数、耗时分别给出可证明的下界/上界。

#### 搜索策略

优先级建议是：

```text
先看是否能比 incumbent 更好
再看 h_pdb
再看 novelty
最后看资源可行性标签
```

这和 Raphael 的差异是：

- Raphael 主要依靠单体 score + 内置 UB/LB。
- 这里是“**incumbent 驱动 + novelty 驱动 + label 驱动**”。

#### CEGAR 细化

两种失败都走 CEGAR：

1. **抽象失败**
   - 抽象规划可行，但 concrete replay 失败。
   - 细化出导致失败的 buff / bucket / combo 维度。

2. **区间失败**
   - 在某些等级成功，在某些等级失败。
   - 把失败等级加入 witness set，再重新搜索。

这能把“过度展开所有等级、所有 buff 组合”的成本，推迟到真正需要的时候。

#### 预计会比 Raphael 更快的原因

- 它会更早得到强 incumbent。
- 它不会在大量等价状态上只靠一个标量分数打转。
- 对常见制作宏，抽象宽度往往比完整状态空间小得多。
- 对区间和多目标问题，Pareto 标签比单一分数更自然。

#### 什么时候不一定赢 Raphael

- 需要极强的数值最优证明，而且状态变化很“发散”时。
- 配方特例很多、抽象特征很难压缩时。
- 用户关闭了模板/DP/学习型 incumbent，只想单次精确求解时。

这组组件仍然有价值，但更适合放在下一节双向 RCSP 主干的外围：

> **如果要做一个更可能在实际中超过 Raphael 的 solver，不是再堆一个更大的单体 solver，而是以双向 RCSP 为主干，再接“快速 incumbent + BFWS/PDB + CEGAR + Pareto labels”的混合框架。**

### 更具体的下一代候选：双向 RCSP 搜索

如果只选一个最像“可落地、且有机会比 Raphael 更快”的核心算法，我现在会优先看**双向资源约束最短路搜索**，也就是 bidirectional A* / bidirectional labeling / pulse 这一脉，而不是纯前向的宽度搜索。

原因很直接：

- Raphael 主要是前向搜索，靠强剪枝和 Pareto 前沿抑制爆炸。
- RCSP 文献里，双向 A* 和双向 pulse 都被证明能在很多实例上明显优于单向 label-setting。
- 制作宏天然有“起点资源”与“终点目标”两侧信息，尤其适合把目标端做成反向需求边界。

对制作宏来说，可以这样落地：

1. 前向维护真实可达状态，像现在一样展开。
2. 后向维护“满足目标所需的最小资源/效果需求”标签，而不是直接反推完整状态。
3. 两边都做 Pareto 剪枝。
4. 在中间层做 meet-in-the-middle，或者让后向标签直接变成前向 A* 的更紧下界。
5. 对等级区间和困难 buff 语义，用 CEGAR 把反向抽象逐步细化。

可实现骨架：

```text
ForwardLabel:
  concrete_state
  prefix_actions
  used_steps / used_time
  consumed_resources

BackwardLabel:
  abstract_requirement_envelope
  suffix_actions_or_pdb_pointer
  remaining_steps_lb / remaining_time_lb
  minimal_required_cp / durability / effect_signature

join(F, B):
  if abstract(F.concrete_state) satisfies B.abstract_requirement_envelope:
    replay F.prefix_actions + B.suffix_actions on concrete settings
    accept only if concrete replay reaches the target
```

后向侧的关键是：它必须是**不会错剪的放松反推**。  
也就是说，反向标签表示“可能有后缀能从这些抽象条件到达目标”的超集。  
如果一个前向状态连这个放松集合都接不上，就可以安全剪枝；如果接上但 concrete replay 失败，就走 CEGAR 细化。

最坏情况下，这套反向抽象可以退化到接近完整状态，因此不牺牲完整性。  
但在常见宏里，后向需求通常只关心 CP、耐久、剩余进展/品质、少数 buff 签名，状态会比完整 `SimulationState` 小很多。

这也能自然覆盖另外两个需求：

- **等级区间**：后向标签从单场景 requirement 变成 witness-level requirement vector。先只放端点或最难等级，候选宏失败后把失败等级加入 witness set。最终输出前必须对完整 `[Lmin, Lmax]` replay。
- **装备最低要求**：后向需求本身就是“达成目标所需的最低资源边界”。固定宏场景可以直接用 replay + 二分；宏未知场景则用这些反向需求给 craftsmanship/control/CP 外层搜索提供更紧的下界。

这条线比单纯的 BFWS 更像一个真正可替代 Raphael 的 exact solver 骨架。  
如果它做得好，BFWS、模板宏、学习型策略都可以退到“incumbent 生成器”的位置，而不是求解主干。

#### Toy 原型信号

为了验证“后向需求标签是否真的能减少前向展开”，我在 `lab/bidir_rcsp_toy.mjs` 做了一个很小的 toy RCSP：

- 状态包含 progress、quality、CP、durability、Veneration、Innovation、Waste Not。
- 目标为 `progress >= 160` 且 `quality >= 130`。
- 最大步数为 15。
- 对照组是只有前向状态展开与分桶 Pareto 去重。
- 实验组额外使用 relaxed suffix table：假设后缀动作享受乐观 buff 效果，生成不会错剪的后向需求上界。

复跑命令：

```bash
/Users/azurice/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node lab/bidir_rcsp_toy.mjs
```

结果：

| 搜索方式 | 找到步数 | expanded | generated | 后向 bound 剪枝 |
| --- | ---: | ---: | ---: | ---: |
| forward only | 13 | 2,281,629 | 14,717,121 | 0 |
| forward + relaxed suffix bound | 13 | 322,717 | 2,416,004 | 1,212,049 |

这个 toy 实验不证明完整 FFXIV crafting 上一定赢 Raphael，但它验证了核心假设：**即使后向需求只是乐观放松，也能在不改变解步数的情况下把前向展开压到约 1/7**。  
下一步应把这个思想接到 Raphael 的真实 `SimulationState` 子集上，先做 progress-only 或 deterministic action subset 的 Rust 原型。

#### Raphael SimulationState 子集原型

进一步，我在 `lab/raphael-rcsp-bound` 做了一个 Rust 原型，直接 path-depend 上游 `raphael-sim` 并调用真实的：

```rust
SimulationState::use_action(action, Condition::Normal, settings)
```

这个原型仍然只覆盖 deterministic 子集：

- `Veneration`
- `Innovation`
- `WasteNot`
- `BasicSynthesis`
- `CarefulSynthesis`
- `BasicTouch`
- `StandardTouch`
- `MasterMend`

实验设置：

- `max_cp = 320`
- `max_durability = 70`
- `base_progress = 20`
- `base_quality = 20`
- `target_progress = 200`
- `target_quality = 180`
- 模板 incumbent 为 12 步，之后测试 `step budget = 11..16`

后向侧仍然是 relaxed suffix table：假设后缀动作享受乐观 buff/IQ 效果，并按最小耐久成本估计。因此它是一个不会错剪的上界过滤器，而不是完整反向模拟。

复跑命令：

```bash
cargo run --manifest-path lab/raphael-rcsp-bound/Cargo.toml --release
```

原型中的模板 incumbent：

```text
StandardTouch, Innovation, StandardTouch, StandardTouch, StandardTouch,
StandardTouch, WasteNot, Veneration,
CarefulSynthesis, CarefulSynthesis, CarefulSynthesis, CarefulSynthesis
```

结果：

时间列是本机 release 单次运行结果，用于判断方向，不是稳定 benchmark。

| step budget | 找到步数 | forward expanded | bounded expanded | 后向 bound 剪枝 | expanded ratio | PDB time | bounded time |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 11 | none | 1,561,793 | 93,982 | 469,171 | 0.060 | 0.736 ms | 32.657 ms |
| 12 | 12 | 2,786,830 | 327,631 | 1,390,647 | 0.118 | 0.979 ms | 124.789 ms |
| 13 | 12 | 2,786,830 | 858,539 | 3,121,418 | 0.308 | 1.059 ms | 359.739 ms |
| 14 | 12 | 2,786,830 | 1,627,983 | 4,413,601 | 0.584 | 1.222 ms | 821.412 ms |
| 15 | 12 | 2,786,830 | 2,257,221 | 4,076,454 | 0.810 | 1.284 ms | 1,409.910 ms |
| 16 | 12 | 2,786,830 | 2,520,876 | 2,905,412 | 0.905 | 1.538 ms | 1,884.763 ms |

这个结果比 toy JS 原型更重要，因为 forward side 已经是真实 Raphael 状态转移。它也暴露出一个关键工程判断：后向需求 bound 最适合和**快速 incumbent**配合使用。先用模板/DP/历史宏拿到一条可行宏，把搜索视界压到 incumbent 步数附近，再用后向 requirement/PDB 证明更短或同长候选；budget 一旦放宽，乐观后缀自然会变得太宽。

其中 `budget = 11` 是更接近真实用途的信号：模板先给出 12 步 incumbent，然后 bounded search 证明“没有 11 步以内解”。这个证明场景中，展开量降到 forward-only 的 `6%`。这正是 incumbent-bound exact search 希望优化的核心路径。

这正好支持“fast incumbent + bidirectional RCSP/PDB”的混合主线。

#### 与 Raphael 现有 UB/LB 的差异

需要注意，Raphael 已经不是单纯前向搜索。它的 `StepLbSolver` 本身就是一种带 `steps_budget` 的放松 suffix DP：

- `ReducedState` 里包含 `steps_budget`、耐久和效果。
- 它在给定步数预算下计算可达 progress/quality 的 Pareto front。
- 主搜索用这个结果估算 `current_steps + remaining_steps_lb`。

所以“后向 suffix bound”这个概念并不是全新发现。真正有新增价值的方向是：

1. **外部 incumbent 驱动**：Raphael 当前只能在主搜索过程中发现解后抬高 `min_accepted_score`。如果先用模板/DP/历史宏给出 incumbent，就能从一开始把搜索限制在短视界附近。
2. **需求标签而不是只有质量/步数上界**：下一代 PDB 应输出 `required_cp`、`required_durability`、`required_effect_signature`、`required_steps` 等 requirement label，前向状态可以直接做 join/prune。
3. **双向 join**：Raphael 现在仍是前向状态为主，UB/LB 是查询器。双向 RCSP 版本会维护一个后向 label frontier，让前向搜索主动和目标侧边界相遇。
4. **区间 witness 与装备逆向复用同一标签**：等级区间可以把 requirement label 扩展成多等级向量；装备最低要求可以直接读取这些 requirement label，减少外层属性枚举。

因此更准确的下一代目标不是“给 Raphael 再加一个 StepLbSolver”，而是把 `StepLbSolver` 这类 suffix DP 提升为 incumbent-bound bidirectional label search 的主干。

#### Raphael 本体对照：外部 incumbent 的直接收益

为了避免只和自写 forward-only 搜索比较，我又在 `lab/upstream/raphael-rs` 的 ignored 上游副本里做了一个实验性 patch：给 `MacroSolver` 增加 `solve_with_initial_solution(initial_actions)`。patch 已保存为：

```text
lab/raphael-rcsp-bound/raphael-solver-initial-solution.patch
```

同一套 `Settings` 和 action mask 下：

| 模式 | solution steps | wall time | inserted nodes | processed nodes | QualityUB states | StepLB states |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Raphael baseline | 12 | 130.035 ms | 3,087 | 430 | 72,590 | 9,533 |
| Raphael seeded incumbent | 12 | 129.585 ms | 794 | 430 | 72,590 | 9,533 |

这个对照有两个含义：

1. Raphael 的现有主搜索已经非常强。在这个小实例上，它的 processed nodes 远低于 lab 原型的 bounded forward search。因此不能把自写 forward-only 的百万级展开当作 Raphael 的真实代价。
2. 外部 incumbent 仍然有直接收益：`inserted_nodes` 从 3,087 降到 794，减少约 `74%`。但现有 `QualityUbSolver` / `StepLbSolver` 预计算不受影响，所以总收益会被预计算成本盖住。

同一原型还记录了用户可感知延迟：

- 模板 incumbent concrete replay：约 `65 us`。
- Raphael baseline 完整求解：约 `130 ms`。
- seeded Raphael 完整求解：约 `130 ms`。
- `budget = incumbent - 1` 的 relaxed PDB 构建 + bounded proof：约 `0.736 ms + 32.657 ms`。

这说明 v3 的关键优势不一定是“单次完整证明总是秒杀 Raphael”，而是**先极快返回 verified incumbent，再异步证明是否可改进**。在小实例上，证明“没有 11 步以内解”的 proof path 也明显短于 Raphael baseline。

随后我把 lab 原型扩成 3 个 scenario，验证这个信号不是单个参数点：

| scenario | target | incumbent | incumbent source | Raphael baseline | seeded inserted | `incumbent - 1` proof |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| baseline | P200/Q180 | 12 | template | 130 ms / 3,087 inserted | 794 inserted | 33 ms / 6.0% expanded |
| higher_quality | P200/Q220 | 13 | Raphael baseline | 152 ms / 3,915 inserted | 835 inserted | 86 ms / 6.4% expanded |
| tighter_progress | P230/Q180 | 13 | Raphael baseline | 143 ms / 14,385 inserted | 4,244 inserted | 76 ms / 6.6% expanded |

解释：

- 这里的 `incumbent - 1 proof` 是证明“没有比 incumbent 更短的解”。
- `expanded %` 是 bounded proof 相对同一 lab forward-only 搜索的 expanded ratio。
- `higher_quality` 和 `tighter_progress` 没有命中当前手写模板，所以用 Raphael baseline 解作为 incumbent 来源；这模拟“任意快速候选生成器已经给出一条 verified incumbent”的后续 proof 阶段。

这个结果强化了 v3 的定位：  
**seeded Raphael 是低风险增量优化；真正可能超过 Raphael 体验和总成本的，是把 incumbent-bound proof 做成 lazy/on-demand，并避免现有全局 UB/LB 预计算。**

因此下一步如果要真正超过 Raphael，优化点应该更精确：

- 第一阶段：给 Raphael 暴露官方 `initial_solution/incumbent` API，先获得低风险收益。
- 第二阶段：让 incumbent bound 参与 UB/LB 预计算，避免为已经不可能打败 incumbent 的状态构建完整 Pareto front。
- 第三阶段：把后向 PDB 从“quality/progress 上界”扩展成 requirement labels，直接用于 CP/耐久/effect join。

更进一步，单纯 seeded Raphael 仍然不是最终形态。因为 baseline/seeded 对照显示：seed 可以减少主搜索插入节点，但不能减少 `QualityUbSolver` / `StepLbSolver` 的全局预计算。  
因此真正更有机会赢 Raphael 的结构应是 **incumbent-first lazy proof**：

```text
1. Fast incumbent phase
   - 模板/DP/历史宏快速生成候选。
   - concrete replay 验证后立即返回给用户。

2. Lazy proof phase
   - 以 incumbent 的 steps/time/quality 作为 bound。
   - 只为 bound 内可能打败 incumbent 的前向状态生成 requirement labels。
   - 后向 PDB 按需展开，而不是像 QualityUbSolver 一样先做全局预计算。

3. Improvement loop
   - 若 proof 内找到更优宏，更新 incumbent 并收紧 bound。
   - 若 bound 内无解，输出“已证明最优/无更短解”。
```

这条线和 ARA*、anytime heuristic search、bounded-cost search 的思想一致：先快速获得可行解，然后把 incumbent 用作后续 exact proof 的约束，而不是等完整预计算结束后才返回结果。

对本项目的意义是：

- 交互体验上可以先返回 verified macro，避免用户等完整证明。
- 算法上避免对和当前 incumbent 无关的状态做完整 UB/LB 预计算。
- 等级区间 proof 可以从少量 witness levels 开始，反例驱动扩张。
- 装备最低要求可以在 proof 过程中顺便收集 requirement labels。

#### v3 算法规格：Incumbent-First Lazy Bidirectional RCSP

当前更清晰的候选算法可以定义为：

```text
1. generate_incumbent()
   - 模板宏、BestCraft Reflect DP、历史宏、贪心策略都可以作为来源。
   - 用 Raphael SimulationState concrete replay 验证。

2. set_bound(incumbent)
   - 若目标是最短满品质宏：bound_steps = incumbent.steps - 1。
   - 若目标是更短耗时：bound_time = incumbent.time - 1。
   - 若允许非满品质：把 quality / steps / time 放进 Pareto incumbent set。

3. build_backward_requirement_pdb(bound)
   - 后向标签不是完整反推状态，而是 requirement envelope：
     required_progress
     required_quality
     required_cp
     required_durability
     required_effect_signature
     remaining_steps

4. forward_exact_search(bound, requirement_pdb)
   - 前向状态仍使用真实 `SimulationState`。
   - 每个前向状态先查 requirement_pdb；连放松需求都接不上就剪。
   - 接得上的候选必须 concrete replay 或继续 exact expansion。

5. certify_or_improve()
   - 如果 bound 内无解，incumbent 在该目标下被证明最优。
   - 如果找到更优解，把它变成新 incumbent 并收紧 bound，循环。
```

这和 Raphael 的搜索方向不同：Raphael 是“从空解开始找好解，过程中逐渐抬高 `min_accepted_score`”；v3 是“先有一个可行解，把 exact search 变成 bounded proof / improvement”。  
这也解释了为什么实验中 `budget = incumbent - 1` 的剪枝最强。

对用户体验也更好：

- 很快给出一条已验证可用宏。
- 后台继续证明是否还能更短/更快。
- 如果证明完成，可以标记“已证明最短”或“当前未证明最优”。
- 对等级区间，incumbent 先做全区间 replay，proof 阶段只在 witness levels 上扩张，失败再加入反例等级。
- 对最低装备要求，requirement label 可以变成属性下界提示，减少外层二分/枚举。

#### v3 的正确性条件

v3 要能宣称“已证明最优”，必须满足这些条件：

1. incumbent 必须通过 concrete replay。对等级区间，则必须对完整 `[Lmin, Lmax]` replay；只通过 witness set 不够。
2. proof 阶段的前向状态必须仍使用真实 `SimulationState` 和真实动作语义。
3. 后向 requirement label 必须是目标可达集合的**超集**。也就是说，它可以过松、可以误放行，但不能错剪任何真实可行后缀。
4. 对每个被剪掉的前向状态，必须满足“它连放松 requirement 都无法接上”。这才是安全剪枝。
5. 如果使用 CEGAR / witness levels，最终证明前必须确保没有未验证等级或未细化抽象会改变结论。
6. 如果目标是最短宏，proof 必须覆盖所有 `< incumbent.steps` 的 action sequence；如果目标是最短耗时，则必须覆盖所有 `< incumbent.time` 的路径；多目标场景则要维护 Pareto incumbent set。

因此 v3 的输出状态应该区分：

- `verified`: incumbent replay 已通过，但尚未证明最优。
- `improving`: 后台 proof 仍在寻找更优解。
- `optimal`: bound 内 proof 完成，已证明无更短/更优解。
- `unknown`: proof 被中断、超时或抽象仍未细化完。

这个语义比传统“求解完成才返回”更适合交互工具。

### Raphael 本身的可优化点

这些优化不改变算法语义：

- 外部 incumbent：允许传入已验证候选宏，提前提高 `min_accepted_score`。
- SearchQueue 避免反复重放：当前节点只存父指针和动作，弹出 batch 时要从初始状态重放动作。可以研究在节点上缓存压缩状态或 periodic checkpoint，但会以更多内存换 CPU。
- 预计算缓存：`FinishSolver`、`QualityUbSolver`、`StepLbSolver` 与 settings 强相关。对于同一配方/属性下调整目标品质、动作 mask 小范围变化时，可研究复用部分预计算。
- 动作 mask 预裁剪：等级区间、禁用专家技能、禁用不可靠动作、禁用高等级技能都会显著减少分支。
- 分层求解：先求 progress-only 或 low-quality feasible 宏，再逐步提高目标品质，可为用户提供渐进结果。

### 不推荐作为主线的方向

- 纯 DFS 或 BFS：完整高等级动作集合下不够稳定。
- MILP/SAT/SMT：可以表达某些有限步问题，但 buff、combo、floor、条件约束会导致模型复杂。适合做离线证明或小步数可行性检查，不适合作为前端交互主 solver。
- 遗传算法/随机搜索：可能很快找到可用宏，但很难给出无解证明和稳定的最短/高质量保证。

## 问题 2：等级同步配方的等级区间支持

结论：可以支持，但要区分两个层级：

1. 只保证“宏里的技能在区间内都能释放”。
2. 保证“同一条宏在区间内每个等级都能完成指定目标”。

第一层很容易接入现有 Raphael。第二层需要候选宏验证，若要严格在求解过程中保证，则需要多场景搜索或迭代求解。

### 上游已有等级同步处理

Raphael 的数据模型包含：

- `Recipe.max_level_scaling`
- `LEVEL_ADJUST_TABLE`
- `RecipeLevelTable`

当 `max_level_scaling != 0` 时，Raphael 用：

```text
effective_recipe_level = LEVEL_ADJUST_TABLE[min(recipe.max_level_scaling, crafter_stats.level)]
```

并且对这类配方把最大耐久修正为 80。

BestCraft 当前 UI 对部分宇宙探索配方提供单个同步等级输入，然后按该等级查询 `RecipeLevelTable` 并生成动态配方。它不是等级区间求解。

本项目已有 `CraftRecipe.max_level_scaling`，但数据包还没有 `LEVEL_ADJUST_TABLE`。要做完整等级同步，建议先导出 `GathererCrafterLvAdjustTable`。

### 保证技能可用

如果用户指定等级区间 `[Lmin, Lmax]`，同一条宏在区间内都能释放技能的充分条件是：

```text
所有技能的 LEVEL_REQUIREMENT <= Lmin
```

落地方式：

1. 计算区间动作 mask：用户选项 mask 与 `LEVEL_REQUIREMENT <= Lmin` 的动作集合取交集。
2. 求解时把 `Settings.job_level` 设为区间内的有效动作等级下限，通常是 `Lmin`。
3. 特殊技能继续按原有选项处理：
   - `Manipulation`
   - `Heart and Soul`
   - `Quick Innovation`
   - `Trained Eye`
   - `Stellar Steady Hand`
4. `Trained Eye` 要额外判断每个等级下是否满足“配方低 10 级以上且非专家配方”。对等级同步配方通常应默认更保守，除非能证明区间每个等级都满足。

这里需要注意，Raphael 当前 `Settings.job_level` 同时控制技能解锁和等级特性。如果游戏里的等级同步会同时压低动作和特性，那么用 `Lmin` 是合理的。如果只想限制动作可用，但保留实际等级的技能特性，则需要在模拟器里拆出两个字段：

- `action_level`
- `trait_level`

FFXIV 等级同步通常更接近前者，即动作和特性都按同步等级处理。

### 保证区间内全部成功

只用最低等级求解不能严格保证所有等级都成功，原因有三类：

- 动态配方的 `RecipeLevelTable` 会随同步等级变化，难度、品质、耐久、divider/modifier 都可能变化。
- 技能特性在等级 11、31、63、82、86、94、96 等点改变状态转移。
- 玩家属性可能随等级或同步规则变化。若没有每个等级的有效属性，就不能证明区间内成功。

建议的产品语义：

```text
levelSyncRange = {
  minLevel,
  maxLevel,
  guarantee: "actionsOnly" | "successAllLevels"
}
```

`actionsOnly`：

- 只限制技能解锁。
- 用区间下限动作 mask 求解。
- 输出宏后只需验证所有动作的等级要求。

`successAllLevels`：

- 对区间内每个等级构造一套 settings。
- 同一候选宏必须在每套 settings 下模拟通过。
- 如果候选失败，求解器必须继续找另一条宏。

### successAllLevels 的实现路线

推荐分两阶段实现。

第一阶段：候选求解 + 全区间验证。

1. 根据用户指定区间构造所有等级的 settings。
2. 选一个代表等级求解。通常先选区间内最难的 recipe level，动作等级仍按 `Lmin` 限制。
3. 得到宏后，对 `[Lmin, Lmax]` 每个等级逐一模拟。
4. 如果全部通过，返回并标记为区间验证通过。
5. 如果失败，报告失败等级，或进入第二阶段。

这个阶段实现成本低，已经能覆盖大量实际场景。因为区间通常最多 100 个整数等级，模拟验证成本很低。

第二阶段：多场景求解。

把搜索状态从单个 `SimulationState` 扩展为多个等级的状态向量：

```text
state_vector = [state_at_level_Lmin, ..., state_at_level_Lmax]
```

每个候选动作必须能在所有场景中执行。终止条件是每个场景都完成进展并达到目标品质。剪枝条件取所有场景的合取：

- `FinishSolver.can_finish` 必须每个场景都为真。
- `QualityUbSolver` 的上界取各场景最小可达目标。
- `StepLbSolver` 的步数下界取各场景最大值。
- Pareto 支配关系按所有场景组件逐项比较。

为了避免状态向量太大，可以先用失败等级做 witness set：

1. 初始只放一个代表等级。
2. 求出候选后验证全区间。
3. 把失败等级加入 witness set。
4. 重新用 witness set 求解。
5. 直到通过或无解。

这类似 counterexample-guided refinement，通常比一开始纳入全区间更快。

### 等级区间的最小数据需求

要在本项目中正确实现，需要补齐：

- 导出 `GathererCrafterLvAdjustTable` 到 `CraftDataPackage`。
- 对 `max_level_scaling != 0` 的配方，根据同步等级选择 `RecipeLevelInfo`。
- 确认等级同步时有效属性如何取值：
  - 如果用户手动输入每个等级属性，按输入构造 settings。
  - 如果只有一套属性，只能保证该属性假设下的区间。
  - 如果要自动按装备同步推导属性，需要额外装备/同步公式数据，本项目当前数据包不包含。

## 问题 3：目标状态逆向求解装备最低要求

结论：可以做，但要先明确“目标状态”和“最低”的定义。

### 固定宏的最低属性

如果宏已经固定，逆向求解很可行。目标可以是：

- 最终 `progress >= recipe.difficulty`
- 最终 `quality >= target_quality`
- 最终 `cp >= target_cp_left`
- 最终 `durability >= target_durability_left`
- 或这些条件的组合

对固定宏，craftsmanship、control、CP 的影响基本是单调的：

- craftsmanship 越高，`base_progress` 越高，进展不会变差。
- control 越高，`base_quality` 越高，品质不会变差。
- max CP 越高，动作越不容易因为 CP 不足失败，CP 恢复上限也更高。

因此可以用二分替代 BestCraft 当前的线性扫描。

推荐算法：

```text
predicate(attrs):
  settings = build_settings(attrs, recipe, level)
  final_state = simulate(settings, macro)
  return final_state satisfies target

min_cp:
  binary search max_cp with craftsmanship/control fixed

min_craftsmanship:
  binary search craftsmanship with control/cp fixed

min_control:
  binary search control with craftsmanship/cp fixed
```

如果要同时求三维最低值，不存在唯一答案。需要选择一种目标：

- 字典序最小，例如先最小 CP，再最小 craftsmanship，再最小 control。
- 加权分数最小，例如按食药/装备成本换算。
- Pareto 前沿，输出多组互不支配的要求。

### 公式反推

Raphael 与本项目使用的基础公式为：

```text
base_progress = floor(craftsmanship * 10 / progress_divider + 2)
base_quality  = floor(control * 10 / quality_divider + 35)

if crafter_level <= recipe_class_job_level:
  base_progress = floor(base_progress * progress_modifier / 100)
  base_quality  = floor(base_quality  * quality_modifier  / 100)
```

单个动作增量大致为：

```text
progress_delta = floor(base_progress * action_mod * effect_mod / 1000)
quality_delta  = floor(base_quality  * action_mod * effect_mod * condition_mod / 20000)
```

理论上可以先反推所需 `base_progress` / `base_quality`，再反推 craftsmanship / control。但由于宏中有 buff、combo、Inner Quiet、提前完成、Trained Eye、对抗质量等状态影响，直接公式反推容易漏边界。

建议实现上仍以二分模拟为主，公式只用于给二分提供更窄的初始上下界。

### 固定宏的注意事项

- 如果 craftsmanship 过高，可能提前完成制作，导致后续动作不再执行。对“最低要求”通常不是问题，但若需要“完整宏步数不变”，还要同时求 craftsmanship 上界。BestCraft 当前就计算了这个上界。
- 如果宏包含 `Trained Eye`，control 下界可能退化，因为它直接填满品质，但要求等级与配方等级差满足条件。
- 如果宏包含条件技能，例如 `Precise Touch`、`Intensive Synthesis`、`Tricks of the Trade`，需要指定是否通过 `Heart and Soul` 保证普通状态可用，否则固定宏在普通条件下不可验证。
- 如果包含随机技能，例如 `Rapid Synthesis`、`Hasty Touch`，Raphael 当前只在 `Stellar Steady Hand` 等保证 100% 成功的情况下使用，否则会视作不可靠动作。

### 非固定宏的最低装备

如果宏也可以变化，问题变为：

```text
是否存在一条宏，使给定属性能达到目标状态？
```

这个谓词仍然大体单调：属性更高不会让可行宏集合变小。但每次判断都需要运行 solver，成本比固定宏高很多。

可行方案：

1. 选定一个标量目标，例如“最小 CP”或“最小总属性分”。
2. 外层二分或 branch-and-bound 枚举属性。
3. 内层调用 Raphael 判断是否存在满足目标的宏。
4. 对找到的宏再运行固定宏逆向分析，收紧该宏的实际最低要求。

更实用的做法是输出 Pareto 前沿：

```text
for cp in plausible_cp_range:
  binary search minimal craftsmanship/control pairs
  keep non-dominated triples
```

但这需要较多 solver 调用。建议作为离线分析或高级功能，不建议一开始放在交互主路径。

### 装备最低要求与具体装备

“最低装备”有两个含义：

1. 最低属性要求：craftsmanship/control/CP 的数值下界。
2. 具体装备组合：哪几件装备、魔晶石、食物、药水能达到这些属性。

本次调研只覆盖第一种。第二种需要装备、禁断、食药、职业共享、物品等级等数据，是另一个约束优化问题。当前项目数据包主要是制作配方、物品和来源，不足以直接求具体装备组合。

## 推荐落地顺序

1. 先补齐数据：导出 `GathererCrafterLvAdjustTable`，并在数据模型中保存 level sync 所需映射。
2. 给 solver API 增加等级区间选项，但第一版只做 `actionsOnly`。
3. 对 `actionsOnly`：用区间最低等级裁剪动作 mask，求解后验证动作等级。
4. 增加固定宏逆向属性分析：用二分模拟求最低 craftsmanship、control、CP，并可选求 craftsmanship 上界。
5. 增加 `successAllLevels` 的候选验证：先求一个候选宏，再模拟区间内所有等级。
6. 如果候选验证失败率高，再实现 witness set 多场景求解。
7. 给 Raphael 适配层增加外部 `initial_solution/incumbent` 实验 API，先把已验证宏注入 `min_accepted_score`。
8. 把 BestCraft Reflect DP、模板库或历史宏作为快速 incumbent 生成器；返回前必须用 Raphael simulator concrete replay。
9. 在 `lab` 里继续做 incumbent-first lazy proof 原型：只为可能打败 incumbent 的状态按需生成 requirement labels。
10. 用 Raphael edge cases 和高等级配方对照 baseline Raphael、seeded Raphael、lazy proof 三组指标：预计算状态数、inserted/processed nodes、wall time、是否证明最优。
11. 如果 lazy proof 在常见实例上稳定减少总耗时或显著提前返回可用宏，再考虑做成可选 solver；否则把 incumbent API 和 requirement labels 作为 Raphael 的启发式增强。

## 当前结论

- Raphael 是当前最值得保留的通用求解核心。BestCraft 自研 DP 可作为快速候选生成器，但不是完整替代。
- 如果要研发“比 Raphael 更优”的下一代 exact solver，目前最具体、最有证据的主线是 **Incumbent-First Lazy Bidirectional RCSP**：先给出 verified incumbent，再用 bound 驱动的按需后向 requirement labels 做最优性证明或改进。
- seeded Raphael 在小型真实 `SimulationState` 子集上能把主搜索 `inserted_nodes` 从 3,087 降到 794，但不能减少现有 QualityUB/StepLB 预计算；因此 lazy proof 的关键价值在于让 incumbent bound 进入预计算/后向标签构建阶段。
- 等级同步的单等级支持已有上游参考；等级区间“动作可用”可以低成本支持；等级区间“全部成功”需要候选验证，严格求解需要多场景搜索。
- 目标状态逆向最低属性对固定宏很可行，建议用二分模拟实现。对宏也未知的场景，可以做外层属性搜索加内层 Raphael，但成本较高，需要先定义优化目标或输出 Pareto 前沿。

### 关键边界：不是“没有更快算法”，而是没有“一招通吃”的更快算法

如果把问题定义为“完整动作集合 + 完整状态语义 + 要求精确/可证无解”，那它就落在经典规划与资源约束最短路一类问题上。  
这类问题本身就是 PSPACE-complete / NP-hard 级别，参考 Bylander 的经典规划复杂度结果，以及 RCSP 的标准 NP-hard 结论。  
所以我不认为会存在一种对所有实例都同时满足“更快、同样完整、同样精确”的统一算法。

这不代表没法更快，只代表“更快”通常来自下面几类变化：

- 限定问题族，例如只做进展、只做固定宏、只做单等级、只做区间 witness。
- 强化启发式，例如 incumbent、PDB、novelty、label-setting。
- 改变求解目标，例如先求可行宏，再求最优宏。
- 利用域结构做抽象，例如常见 skeleton、buff 组合、等级段。

所以更准确的表述是：

> 对完整通用问题，没有看到能稳定、统一地压过 Raphael 的单体算法；
> 但对常见实例、受限子问题和工程化路径，仍然有明确的提速空间。

换句话说，真要“超越 Raphael”，大概率不是再造一个更大的 DFS/DP，而是做一个以 incumbent-first lazy proof 为主干、再接双向 RCSP / pulse、requirement PDB、CEGAR 的混合框架。
