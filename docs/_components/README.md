#import "pele/lib.notc" as pele;
#import "typst/lib.notc" as typst;
# 文档组件

Pele 与 Typst 组件整体复制自 `icy-bot/docs/_components/`，包含声明、浏览器实现、说明与测试。

## Pele

在 Notist Markdown（`.md`）文档中导入组件后，使用 Mermaid 语法绘制图表：

#pele.diagram(r#"""
flowchart LR
    edit[编辑文档] --> build[构建站点]
    build --> preview[预览页面]
    preview --> edit
"""#)

完整说明见 [Pele](pele/README.md)。

## Typst

数学示例：#typst.math("a^2 + b^2 = c^2")。

`Notist.toml` 同时将内置 `notist.math` 替换为 `typst.math`。通用 SVG 与公式的更多用法见 [Typst](typst/README.md)。

## 网络依赖

组件在浏览器中加载远程 JavaScript、WASM 或字体等资源，首次渲染需要联网。Notist 静态构建成功不等同于离线可用。
