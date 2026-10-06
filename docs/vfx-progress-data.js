window.VFX_PROGRESS = {
  "schemaVersion": 2,
  "updatedAt": "2026-10-06T21:24:20+08:00",
  "state": "幻化 / VFX 已整合到本地 dev",
  "latestAuditCorrection": "合入目标纠正为dev；本地main已恢复ce20f59。首版范围保持不变，完整覆盖留待后续。",
  "currentAudit": "整合版workspace共1799项通过、0失败、356忽略；WASM编译通过。原生多实例+VFX、真实剑盾rest/动画及16053/16063挂载1x/4x均通过，共24份定向快照。",
  "activity": "整合与验收完成，已合入本地dev，尚未推送。原dev的28个和glamour的23个工作文件保持原样；幻化页自动加载武器VFX仍待接入。",
  "summary": "首版提供常见武器特效的近似预览，保留明确降级提示。核心可用性验证已通过；完整tone-map、雾、所有字段及游戏画面一致性不阻塞本版。",
  "focus": {
    "state": "verified",
    "title": "dev 整合幻化与 VFX",
    "status": "已通过验收并合入本地 dev",
    "description": "glamour的套装管理、逐件换装、染色、骨架与体型缩放已与VFX首版整合。统一IMC和场景渲染入口；原glamour工作区保留。幻化页自动武器VFX尚未接入。",
    "resource": "docs/dev-glamour-vfx-integration.md",
    "steps": [
      {
        "state": "verified",
        "label": "保留源工作区并整合冲突",
        "detail": "23个原glamour修改/未跟踪文件保持不变；9个冲突文件已解决。"
      },
      {
        "state": "verified",
        "label": "普通workspace回归",
        "detail": "1799项通过、0失败、356忽略；data1449、renderer203，忽略项不计为通过。"
      },
      {
        "state": "verified",
        "label": "WASM与原生GPU验收",
        "detail": "WASM编译、多实例+VFX 1x/4x、真实剑盾rest/动画及16053/16063挂载1x/4x通过；24份定向快照，独立日志保留。"
      },
      {
        "state": "verified",
        "label": "合入本地 dev",
        "detail": "整合提交合入本地dev；main保持ce20f59，远端尚未推送。"
      }
    ],
    "images": [
      {
        "src": "vfx-progress-assets/v1-lance-1x.png",
        "title": "屠龙戟·灵光 #16053",
        "detail": "原生RTX 4070 Ti SUPER，真实资源逐帧播放0.8秒。展示本版近似效果，不代表游戏同条件画面对照。"
      },
      {
        "src": "vfx-progress-assets/v1-shield-1x.png",
        "title": "圣母盾·灵光 #16063",
        "detail": "原生RTX 4070 Ti SUPER，真实资源逐帧播放0.8秒。展示本版近似效果，不代表游戏同条件画面对照。"
      },
      {
        "src": "vfx-progress-assets/dev-dressed-sword-shield-rest.png",
        "title": "敖龙女 · 剑盾穿搭 Rest",
        "detail": "整合版RTX 4070 Ti SUPER实装资源原生快照；七件装备与身体独立实例。取景仍使用未缩放bounds，超长武器可能超出画面。"
      },
      {
        "src": "vfx-progress-assets/dev-dressed-sword-shield-animated.png",
        "title": "敖龙女 · 剑盾随角色动画",
        "detail": "整合版RTX 4070 Ti SUPER实装资源原生快照；七件装备与身体独立实例。取景仍使用未缩放bounds，超长武器可能超出画面。"
      }
    ]
  },
  "metrics": [
    {
      "label": "本次目标",
      "value": "已合入本地 dev",
      "detail": "幻化与VFX首版已整合；长期全覆盖及幻化自动武器VFX继续维护。"
    },
    {
      "label": "首版真实样本",
      "value": "2 / 2",
      "detail": "16053、16063，原生1x和4x均通过。仅代表这些验收样本。"
    },
    {
      "label": "首版合入检查",
      "value": "6 / 6",
      "detail": "本版列明的合入条件通过；不表示8个覆盖领域全部完成。"
    },
    {
      "label": "游戏画面一致性",
      "value": "后续验收",
      "detail": "亮度、材质与场景仍有差异，不计为当前进行中的合入阻塞。"
    }
  ],
  "tasks": [
    {
      "id": "preview",
      "state": "verified",
      "area": "首版",
      "title": "稳定预览与降级",
      "detail": "模型与特效可显示；初始化错误可见，VFX相机失败不再中断基础模型。",
      "evidence": [
        {
          "label": "首版范围与验收",
          "href": "vfx-v1.md"
        }
      ]
    },
    {
      "id": "sample-gpu",
      "state": "verified",
      "area": "首版",
      "title": "代表性真实资源验收",
      "detail": "屠龙戟·灵光和圣母盾·灵光，真实挂载逐帧播放，1x/4x通过。",
      "evidence": [
        {
          "label": "首版范围与验收",
          "href": "vfx-v1.md"
        }
      ]
    },
    {
      "id": "diagnostics",
      "state": "verified",
      "area": "首版",
      "title": "可理解的预览差异",
      "detail": "重复项按类型汇总，说明视觉影响，详情保留资源和粒子编号。",
      "evidence": [
        {
          "label": "首版范围与验收",
          "href": "vfx-v1.md"
        }
      ]
    },
    {
      "id": "verification",
      "state": "verified",
      "area": "首版",
      "title": "构建与回归",
      "detail": "普通workspace、WASM、Chromium shader及定向原生验证通过。",
      "evidence": [
        {
          "label": "首版范围与验收",
          "href": "vfx-v1.md"
        }
      ]
    },
    {
      "id": "coverage",
      "state": "verified",
      "area": "首版",
      "title": "首版范围与后续覆盖",
      "detail": "当前覆盖和缺口分开维护，不再以全部字段覆盖作为合入条件。",
      "evidence": [
        {
          "label": "首版范围与验收",
          "href": "vfx-v1.md"
        }
      ]
    },
    {
      "id": "dev-integration",
      "state": "verified",
      "area": "合入",
      "title": "首版合入本地 dev",
      "detail": "2026-10-06：583065c已快进合入本地dev，无冲突；验收源码摘要一致。main恢复ce20f59，origin/dev尚未推送。",
      "evidence": [
        {
          "label": "合入清单",
          "href": "vfx-v1.md#merge"
        }
      ]
    },
    {
      "id": "glamour-integration",
      "state": "verified",
      "area": "整合",
      "title": "dev 整合幻化与VFX",
      "detail": "9个冲突文件已解决，统一IMC和场景渲染；普通workspace、WASM与定向原生GPU验收通过，已合入本地dev。幻化自动武器VFX另列待办。",
      "evidence": [
        {
          "label": "整合范围与验收",
          "href": "dev-glamour-vfx-integration.md"
        }
      ]
    }
  ],
  "coverage": [
    {
      "title": "武器挂载与资源",
      "level": "首版可用",
      "supported": "IMC→AVFX、主副手挂载、贴图和绘制模型、基础骨架/绑点；资源失败保留诊断。",
      "remaining": "幻化页IMC→AVFX资源加载与动画武器宿主接入、安装全集验收、其它宿主资源与动态游戏来源。"
    },
    {
      "title": "常见粒子与网格",
      "level": "部分覆盖",
      "supported": "Quad、Model/LightModel及多种Laser/Disc/Polygon/Line/Polyline/Powder/Windmill子集已有解析、播放或绘制；本版实际样本覆盖点状粒子和网格火焰。",
      "remaining": "各族完整Draw、部分轴序扰动/LOD、Smpl与provider边界；不按枚举出现即视为完全支持。"
    },
    {
      "title": "ModelSkin与Decal",
      "level": "部分覆盖",
      "supported": "武器/副手Aura选择、资源绑定、部分Fresnel/UV；Decal已有部分前向深度接收路径。",
      "remaining": "角色/召唤兽表面宿主、完整多目标、deferred GBuffer与水面模板。"
    },
    {
      "title": "曲线与随机",
      "level": "部分覆盖",
      "supported": "常用动画、构造/属性缓存与预览共享随机流；相同输入可重播，保留已有精确数值回归。",
      "remaining": "完整全局创建顺序、其它随机消费者、运行时真实TLS与极端值域。"
    },
    {
      "title": "调度与绑定",
      "level": "部分覆盖",
      "supported": "逐帧播放、部分Scheduler/Timeline/Item、骨架与预览相机接线、Reset及受限控制。",
      "remaining": "所有Binder/Clip工厂、真实角色运动及完整生命周期组合。"
    },
    {
      "title": "场景与亮度",
      "level": "近似预览",
      "supported": "已有混合、深度、软粒子、部分反射/法线采样；初始化限额和WGSL浏览器编译已验证。",
      "remaining": "逐粒子tone-map、雾、全部光照、曝光及完整场景合成。蓝色火焰可能过亮；盾面材质仍有近似差异。"
    },
    {
      "title": "Effector与其它宿主",
      "level": "后续版本",
      "supported": "相关格式已有部分解析及能力诊断。",
      "remaining": "完整灯光、镜头震动、后处理、非武器VFX宿主。"
    },
    {
      "title": "游戏画面一致性",
      "level": "后续验收",
      "supported": "用户已确认网页可见；两份真实资源的原生1x/4x可见性和普通回归通过。",
      "remaining": "同视角、同时间、同场景的游戏动态/像素对照；首版不承诺复刻全部效果。"
    }
  ],
  "checks": [
    {
      "name": "代表性真实武器",
      "result": "16053 / 16063 · 原生1x/4x通过",
      "scope": "NVIDIA RTX 4070 Ti SUPER / Vulkan，按网页逐帧输入并选择Aura；两种MSAA共16份快照，实际特效区别于无VFX基线。",
      "href": "dev-glamour-vfx-integration.md#验证"
    },
    {
      "name": "幻化与VFX共享场景",
      "result": "原生多实例与剑盾动画通过",
      "scope": "NVIDIA RTX 4070 Ti SUPER / Vulkan，多实例+VFX 1x/4x共6份快照；七件装备与身体独立实例、剑盾rest/动画共2份快照。共享渲染器验收，幻化页自动加载武器VFX仍待接入。",
      "href": "dev-glamour-vfx-integration.md#验证"
    },
    {
      "name": "普通测试与网页编译",
      "result": "全部通过",
      "scope": "整合版workspace game-data/render-test-support/web：1799通过、0失败、356忽略；data1449、renderer203及应用/集成用例通过。忽略项不计通过；Web/WASM编译通过。",
      "href": "dev-glamour-vfx-integration.md#验证"
    },
    {
      "name": "浏览器shader与初始化",
      "result": "Chromium编译通过",
      "scope": "旧WGSL逐行复现906:26；五模块修正后零错误。原生1x/4x限额与20份法线/反射像素回归通过。用户最新截图确认模型和特效可见。",
      "href": "vfx-v1.md#verification"
    },
    {
      "name": "诊断与记录",
      "result": "分组回归通过",
      "scope": "跨粒子/附件合并同类差异，保留参数差别和完整资源来源；进度页加载、失败保留与恢复验证通过。",
      "href": "vfx-v1.md"
    }
  ],
  "references": [
    {
      "label": "当前dev整合范围与验收",
      "href": "dev-glamour-vfx-integration.md"
    },
    {
      "label": "当前dev整合源码与日志摘要",
      "href": "dev-glamour-vfx-verification.json"
    },
    {
      "label": "首版范围、命令和已知差异",
      "href": "vfx-v1.md"
    },
    {
      "label": "首版候选历史验证与源码摘要",
      "href": "vfx-v1-verification.json"
    },
    {
      "label": "详细研究历史",
      "href": "vfx-progress-history-2026-10-04.json"
    }
  ],
  "updates": [
    {
      "title": "2026-10-06 整合幻化与VFX",
      "detail": "将glamour当前开发快照11c7968f与VFX首版在本地dev整合，解决9个冲突文件。1799项普通测试、WASM及定向原生GPU回归通过；原工作区保留，尚未推送。"
    },
    {
      "title": "2026-10-06 首版合入本地 dev",
      "detail": "合入目标纠正为dev：候选583065c快进合入，main恢复ce20f59。原dev工作区的28个修改/未跟踪文件原样保留在wip/dev-before-vfx-preview-v1，远端尚未推送。"
    },
    {
      "title": "2026-10-04 收紧首版范围",
      "detail": "从全覆盖改为可用预览与可合入候选；首版条件与长期覆盖分列。"
    },
    {
      "title": "2026-10-04 真实逐帧验收",
      "detail": "修正离屏用例的时间输入与Aura选择以匹配网页。16063旧失败被正确输入消除；没有删除像素可见性断言。"
    },
    {
      "title": "2026-10-04 浏览器初始化与诊断",
      "detail": "资源限额与WGSL一致性两项回归已修复；同类预览差异按类型汇总。"
    }
  ]
};
