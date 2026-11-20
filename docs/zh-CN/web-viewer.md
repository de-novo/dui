# Web 查看器架构

[English](../en/web-viewer.md) | 简体中文 | [한국어](../ko/web-viewer.md)

## 1. 范围与入口

本文规定 v0.0.2 的实现方式。当前由 `crates/dui` 写入独立 HTML 图库，`dui open` 打开历史文件。新查看器位于 `apps/canvas`，使用 TypeScript 7、React 19、TanStack Start 的 SPA 模式、shadcn/ui、Tailwind CSS 和 React Flow。这些依赖归查看器所有，被捕获项目使用自己的技术栈。

构建 dui 时生成客户端，将入口页与带哈希的资源嵌入 Rust 二进制。源码安装与发行构建共用该构建过程。只需 dui 和浏览器即可打开归档。保留旧独立 HTML 图库。实时会话还需要目标工具链和渲染器。[核心架构](architecture.md) · [里程碑](roadmap.md#v002)

**商业分发。** 地图使用 MIT 许可的 `@xyflow/react`，保留默认署名。dui 的付费版本可以使用该库，无需订阅 Pro。上游请求隐藏界面署名的项目订阅 Pro。分发内嵌查看器的二进制时，还须附上实际使用版本的版权声明和完整 MIT 许可证。更新锁文件时检查打包依赖。使用公开 API 实现地图功能，仓库不收录 Pro 示例或模板。 [MIT](https://github.com/xyflow/xyflow/blob/main/LICENSE) · [ProOptions](https://reactflow.dev/api-reference/types/pro-options) · [React Flow Pro](https://reactflow.dev/pro/content)

### React Flow 验证

开发预览从已完成的运行中读取入口截图和发现的链接。在仓库根目录使用 Node 26 运行下列命令，打开输出的回环地址。将最后的路径替换为捕获结果的输出目录。

```sh
npm ci --prefix apps/canvas
npm --prefix apps/canvas run preview:archive -- /path/to/run
```

使用 npmgraph 归档确认了两个界面节点、一条发现的链接，以及 375 和 1280 像素的四张截图。Chrome 检查覆盖键盘选择、焦点恢复、刷新、后退和默认署名。此预览仅验证地图组件，Start 应用和实时详情将在后续实现。已安装的 CLI 仍打开独立图库。

新地图中，若已捕获页面的 DOM 含有指向另一已捕获页面地址的同源链接，则绘制有向边。重复链接合并为一条边，仅在观测到该链接的宽度显示。它说明页面提供此导航选项；验证实际跳转还需回放。

预览可接收一个或多个运行目录。省略参数时从 `~/.dui/projects` 查找已保存的界面地图。在 **Select workspace** 中选择项目、已记录的分支和版本。侧边栏依次提供工作区选择、主题和折叠。**Map options** 提供界面查找、连接模式和捕获宽度。选择卡片可查看详情及已观测到的连接。卡片统计已有状态和宽度中的未验证与无法渲染格子数。

### 查看器设计系统

预览使用 Tailwind CSS 4.3.5 和 shadcn/ui 的 Base UI 预设（`base-nova`）。通用按钮与对话框由 shadcn CLI 4.23.0 生成，运行时使用 `@base-ui/react` 1.9.0。Tailwind 与屏幕地图共用主题变量。

[tokens.css](../../apps/canvas/src/styles/tokens.css) · [Button](../../apps/canvas/src/components/ui/button.tsx) · [Dialog](../../apps/canvas/src/components/ui/dialog.tsx) · [CaptureCard](../../apps/canvas/src/features/map/CaptureCard.tsx)

- **色彩与材质** — 两种主题的画布和面板均使用不透明的黑白表面。橙色 `--primary` (#EF803F) 用于 dui 标记和主要操作。仅固定侧栏和截图卡片使用 `--blur` (10 px)。
- **字体与间距** — 控件使用系统无衬线字体，路径使用系统等宽字体。默认字号下，元信息、正文和面板标题分别为 12 / 14 / 20 px，间距使用 4 / 8 / 12 / 16 / 24 px。控件圆角为 6 px，面板为 12 px。
- **控件** — 面板操作复用 `Button` 的默认、主要或图标样式。纯图标控件需提供无障碍名称。轮廓线用于键盘焦点，位置和字重表示选中状态。
- **截图与判定** — 两种地图视图均使用 `CaptureCard`。保持原始宽高比，路径名称位于截图下方。卡片显示待检查的格子数，详情面板显示记录的判定和原因。
- **导航** — 侧栏顺序固定为工作区、主题、折叠。界面查找位于地图中。选择界面可放大截图，每页显示两个观测到的相邻界面。关闭详情可返回原地图视口。

在实际查看器中检查 375 和 1280 px 下两种主题的总览和选中界面。确认可以访问所有相邻界面，箭头方向与归档相符，截图保留原色，关闭详情后键盘焦点复原。

## 2. 模块与依赖

```text
apps/canvas/src/
  routes/                        # TanStack Start
  features/{history,map,inspector}/
  model/                         # selection, records, projections
  data/                          # HTTP, archive decoding, session client
  components/ui/                 # shadcn/ui primitives
  styles/                        # shared Tailwind tokens
packages/schemas/                # archive and session JSON Schemas
```

路由组合功能并校验 URL 参数。功能模块依赖共享模型和基础控件，通过选择项或带类型的回调通信。模型只含纯函数与不可变值，不依赖 React 或 I/O。只有地图把记录转换为 React Flow 节点和边。首次打开时加载地图与详情模块。生成的 shadcn/ui 代码和主题变量仅用于查看器。

Rust 负责验证、覆盖率和归档写入。CLI 与 MCP 调用相同的应用操作。本地 HTTP 适配器向查看器提供数据。在核心架构规划的 crate 拆分之前，这些外层职责放在 `crates/dui`。保留已有的三个端口。功能有了第一个调用方时才创建模块。

### 2.1 整洁架构边界

按规则的归属组织变更。源码依赖指向内层。运行时需要外部工具时，通过内层定义的端口调用。

| 边界 | 职责 | 允许的依赖 |
|---|---|---|
| 领域 | 带类型的观测值、格子判定与会话决策。相同输入产生相同结果。 | `dui-core` 内的纯值和函数。 |
| 应用 | 查询、编辑和保存用例。协调命令顺序，调用端口，把观测值交给领域，返回带类型的结果。 | 领域及 `TypeOracle`、`Host`、`Renderer` 契约。 |
| 适配与组装 | CLI、MCP、HTTP 转换输入及错误。编译器和 Chromium 适配器收集证据。二进制入口连接具体实现。 | 应用或端口契约、领域值及各适配器所用的工具。 |
| 浏览器呈现 | URL 选择、视图模型和控件。路由加载器通过 `data/` 读取，组件接收模型值和回调。 | `data/` 将传输类型转为 `model/` 的值，`features/` 呈现模型。模型不依赖这两层。 |

**一次编辑的路径。** CLI 和 MCP 将请求解码为同一个编辑命令。用例请求领域作出决策。过期修订号返回冲突，对渲染器的调用为零。获准的编辑产生待执行的效果。应用通过渲染端口执行，将观测结果转换为领域值后返回判定。仅由传输适配器将冲突映射为相应协议错误。用内存渲染器测试拒绝路径，用普通观测值测试判定。

**迁移位置。** 当前 `ports.rs` 传递 JSON 值，`analysis.rs` 在核心验证结束后补充 `missing-screen-assertion`。实现实时验证时，将该判定移入共享验证器，在适配边界转换有关 JSON 字段。保持 CLI 和 MCP 结果兼容。领域输入不得包含 React Flow 节点、CDP 响应或 HTTP 状态码。纯操作使用普通函数，trait 仅用于已有端口边界。

## 3. 数据与导航

以 `packages/schemas` 中的公开 JSON Schema 作为传输契约。按契约检查 Rust 输出，并生成 TypeScript 传输类型。外部 JSON 在统一入口解码为查看器模型。允许新增字段。用旧格式解码器读取当前无版本的归档，新归档记录模式版本。版本不受支持或必填字段无效时，只对该次运行显示数据错误，其他记录仍可打开。

已保存的运行记录不可变。标识和 Git 信息来自 `snapshot.json`，格子的证据来自 `report.json`，连接结构来自 `graph.json`。在所选运行内查询不透明 ID。分支与工作树用于筛选历史。路由为 `/projects/$project/runs/$run`，校验查询键 `screen`、`state`、`width` 和 `view`。省略可选键可打开总览。列表、地图和详情共用同一选择项。后退、刷新或通过键盘返回后恢复选择。缺失的格子显示为不可用。

Rust 监听回环地址。在 `/api/runs/$project/$run` 提供已完成的运行，通过该运行内的引用查找图像。查看器导航返回静态入口页。缺失的 API 记录或资源分别返回对应错误。实时修改权限限于所属本地会话。导航 URL 不包含控制凭据或文件系统路径。

## 4. 实时编辑与失败处理

一个本地 dui 进程持有实时会话的宿主与渲染器。CLI 和 MCP 通过会话 ID 访问该进程。查看器通过回环地址读取同一会话的修订号和最近观测的画面。已保存的图像标为归档。实时详情展示运行中目标的最近观测帧和编辑状态。修订号变化时刷新，丢弃迟到的旧响应。

使用核心的 Decider（命令 → 事件与效果），外层负责释放资源。操作为 `open → inspect → set/reset → save → close`，其中查询、编辑、保存可重复。查询返回所有者、实例、源码位置、类型、当前值和可编辑性。每次编辑带上会话、格子、实例、预期修订号和请求 ID。同一会话的编辑串行处理。类型无效、实例不明确或修订号过期时，在写入前拒绝。同一请求重试时返回已记录的结果，同一 ID 携带不同内容时拒绝。

写入后回读值，并核对 DOM 或无障碍树的可见结果。写入后验证失败时返回观测值和原因，递增修订号并标记未验证。渲染器断开时使会话失效，可从归档格子重开。保存时原子地发布新的本地运行记录。关闭时释放持有的渲染器与宿主。切换格子不会改变正在执行的编辑的目标。

首个客户端启动同一二进制的后台实例。在 `~/.dui` 中使用用户私有锁协调启动，绑定成功后写入回环地址。后续客户端复用该实例。会话 ID 失效后需重新打开格子。查看器通过同源请求读取 JSON，CLI 和 MCP 发送经过认证的本地命令。详情打开时轮询会话修订号，关闭后停止。

```text
open(project, run, cell, requestId) -> session, revision
inspect(session, cell) -> revision, controls[], observation
set(session, cell, instance, slot, value, expectedRevision, requestId) -> editResult
reset(session, cell, instance, slot, expectedRevision, requestId) -> editResult
save(session, cell, expectedRevision, requestId) -> project, run, cell
close(session) -> closed
editResult = {session, cell, instance, slot, revision, observedValue, status, reason?, evidence}
```

重置将所选控件恢复到会话打开时的值。保存时记录最近观测的修订号及验证结果。先检查请求 ID，再检查预期修订号，因此响应丢失后重试仍返回首次结果。请求结果保留至会话关闭，所属进程重启后旧会话失效。若目标应用的版本与归档不同，则单独显示实时运行的来源。

## 5. 模式与验收

| 模式 | 验收结果 |
|---|---|
| 边界解码与纯函数投影 | 读取新旧归档，地图与列表解析到相同格子。未知状态显示数据错误。三种判定及原因均由 Rust 提供。 |
| 功能内临时状态与共享选择 | 仅用键盘进入详情并返回。后退恢复焦点和选择，刷新保留已选宽度。 |
| 组合无障碍控件 | 检查已验证、未验证和不可渲染格子。在 375 和 1280 像素下均可读到状态与原因，加载占位图保留记录的判定。 |
| 单会话串行命令 | 用 CLI 和 MCP 编辑布尔值和枚举。检查独立控件、重复请求、过期编辑、断线和重置，同时核对回读值与可见证据。 |
| 内嵌客户端与不可变归档 | 从干净的检出安装。在目标应用停止时直接打开详情链接。缺失图像显示错误。保存实时状态后，新旧记录均可重新打开。 |

实施顺序为归档和模式夹具 → 路由与可键盘操作的列表 → 地图 → 实时查询与编辑 → 打包和测量。使用 npmgraph 的两个真实界面，以及包含两个独立控件的可再分发夹具。测量 30 次热编辑，期间宿主重建和浏览器重启均为零。分别报告冷启动时间与热编辑的 p50、p95。[shadcn/ui](../../.agents/skills/shadcn/SKILL.md)
