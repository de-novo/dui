# dui 源码模型

[English](../en/source-model.md) | 简体中文 | [한국어](../ko/source-model.md)

> 本文是[英文原文](../en/source-model.md)的简体中文翻译。如有出入，以英文为准。

> 状态：schema 版本 1，2026-11-27 决定（[#353](https://github.com/de-novo/dui/issues/353)）。[默认流水线](roadmap.md#default-pipeline)的第 1 阶段。相关：[架构](architecture.md) · [领域模型](domain-model.md) · [路线图](roadmap.md)。

源码模型是源码对其 UI 的声明。语言适配器根据语法写出模型，官方编译器可以对其细化。近似渲染器、热重载、槽位与状态切换以及之后的导出只读取这个模型，因此新语言只需要一个写出该模型的适配器。

| 部分 | 文件 |
| --- | --- |
| JSON Schema | [`schemas/source-model.v1.schema.json`](../../schemas/source-model.v1.schema.json) |
| Rust 类型与读取器 | [`crates/dui-core/src/source_model/`](../../crates/dui-core/src/source_model/mod.rs) |
| 参考示例 | [`fixtures/source-model/src/`](../../fixtures/source-model/src/) 中 TSX 源码对应的 [`fixtures/source-model/model.json`](../../fixtures/source-model/model.json) |
| 检查 | `npm run check:source-model` |

## 1. 原则

1. **声明，而非观察。** 模型只记录代码所说的内容。它不运行处理函数，不调用 API，也不启动浏览器。捕获结果留在[捕获上下文](domain-model.md#2-实体值对象与标识)中。
2. **不编造。** 只有运行时才知道的值是 `opaque` 表达式。适配器读不出的类型是 `unknown`。无法映射的语法是 `placeholder` 节点。无法解析的样式是 `unresolved` 值或 `unresolvedStyle` 条目。每一项都保留源码文本或原因。
3. **每个事实都有位置。** 界面、组件、槽位、节点、样式、token、列表项 schema 和边都带有源码位置。可能来自不同来源的事实带有 `origin`：`syntax`、`compiler` 或 `inferred`。`inferred` 表示由 AI 或启发式填补的空缺。
4. **确定性。** 相同的源码产生相同的字节。模型没有时间戳，也没有绝对路径。顶层列表按标识符排序。子节点保持源码顺序。写入方省略缺失的可选字段、`false` 标志以及空列表和空映射。
5. **语言中立。** 布局、状态、导航和样式使用中立术语。每个适配器把自己的语法映射到这些术语上。

## 2. 文档

```mermaid
flowchart LR
  F[files] -.-> L((locations))
  S[screens] --> C[components]
  C --> SL[slots]
  C --> N[node tree]
  N -->|instance| C
  N --> ST[style declarations]
  ST --> T[tokens]
  SL --> IS[itemSchemas]
  N -->|list itemType| IS
  E[edges] -->|from| N
  E -->|to| S
```

| 字段 | 内容 |
| --- | --- |
| `schemaVersion` | `1` |
| `extractor` | 写出模型的适配器的 `name` 和 `version` |
| `files` | 相对项目的 `path`（`/` 分隔）、`language`（`typescript`、`swift`、`kotlin`、`dart`、`css`）、所读字节的 `sha256`，以及它 `imports` 的项目文件。热重载使用这些摘要和导入图 |
| `screens` | `id`、`name`、路由器声明时的 `route`、根 `component`、`origin`、`location` |
| `components` | `id`、`name`、`location`、`slots`，以及一个 `root` 节点 |
| `itemSchemas` | 集合项的形状：`id`、`name`、`fields`（`name`、`type`、`optional`）、`origin`、`location` |
| `tokens` | 设计 token：`id`、按原样书写的 `name`、`group`、`value`、可选的 `modes`、`syntax`，以及 `location` 或 `package` |
| `edges` | 导航：`id`、触发节点 `from`、`kind`、`to`、`origin`、`location` |
| `diagnostics` | 适配器要报告的其他内容：`code`、`message` 和可选的 `location` |

**位置**由 `file`、从 1 开始的 `line` 和 `column`（column 按 Unicode 标量值计数），以及从 0 开始的 UTF-8 字节偏移 `start` 和 `end`（不含 `end`）组成。

## 3. 标识符

标识符是不透明的字符串。只有适配器创建标识符；只要声明的文件、名称和在树中的位置不变，标识符就必须不变，这样热重载才能只修补变化的部分。参考示例使用下面的模式，其他适配器也应遵循。

| 实体 | 模式 | 示例 |
| --- | --- | --- |
| Component | `<file>#<name>` | `src/Orders.tsx#Orders` |
| Slot | `<component>.<name>` | `src/Orders.tsx#Orders.filter` |
| Node | `<component>@<path>`：从根开始的子节点索引，分支 arm 计为一层 | `src/Orders.tsx#Orders@0.2.1.0` |
| Screen | `route:<path>`，根视图使用组件 ID | `route:/settings` |
| Item schema, token | `<file>#<name>`，框架默认值使用 `<package>#<name>` | `tailwindcss#--text-2xl` |
| Edge | `<trigger node>:<kind>` | `src/Orders.tsx#Orders@0.3:link` |

带有 `route` 的界面通过路径对应到捕获上下文的 `ScreenId`，因此可以在模型旁边展示可选的捕获依据。

## 4. 槽位与表达式

**槽位**是组件读取、审阅者可以设置的值：`prop`、已声明的 `local` 状态，或来自 `store`、`context`、`route`、`query` 或其他 `external` 调用的数据。它就是术语表中的[状态槽](../i18n.md#glossary)。槽位有 `type`、可选的 `initial` 值（local 状态的初始值或 prop 的默认值）、prop 的 `optional`，以及非 local 数据读取的 `source` 文本。

| 类型 `kind` | 含义 |
| --- | --- |
| `boolean`、`string`、`number` | 基本值 |
| `enum` | 字面量 `values` 的封闭集合 |
| `array` | `item` 的列表 |
| `object` | 由列表项 schema `schema` 描述的对象 |
| `node` | 传入的 UI 内容（children、`ReactNode`、`@ViewBuilder`） |
| `function` | 回调 |
| `unknown` | 无法读取；可选的源码 `text` |

**表达式**说明值从哪里来。

| 表达式 `kind` | 含义 |
| --- | --- |
| `literal` | 源码中写出的 JSON `value` |
| `slot` | 读取同一组件的槽位，可带字段 `path`，例如 `["length"]` |
| `item` | 读取外层 `list` 节点的当前项，并带字段 `path`。这就是字段到 UI 的绑定 |
| `opaque` | 模型不求值的代码：其 `text` 以及其中的槽位与列表项 `reads` |

## 5. 节点

| 节点 `kind` | 字段 | 含义 |
| --- | --- | --- |
| `element` | `tag`、`module`、`props`、`style`、`unresolvedStyle`、`children` | 平台元素，或模型未描述的某个包（`module`）中的组件。已变成样式的属性不再重复写入 `props` |
| `instance` | `component`、`props`、`children` | 对模型中组件的使用。prop 名称与目标的 `prop` 槽位一致 |
| `text` | `parts` | 由字面量片段和绑定片段组成的文本 |
| `branch` | `on`、`arms` | 依赖某个值的 UI：`if`、`&&`、三元表达式和 `switch`。每个 arm 有 `when` 和 `children` |
| `list` | `source`、`item`、`itemType`、`key`、`children` | 为每一项重复的 UI。`children` 是单项模板 |
| `outlet` | `slot` | 绘制 `node` 槽位所持有的 UI 内容 |
| `placeholder` | `reason` | 适配器无法映射的源码 |

每个节点都有 `id` 和 `location`。分支 arm 的 `when` 是 `{"equals": <value>}`、`{"truthy": true|false}` 或 `"otherwise"`。`otherwise` 放在最后。审阅者可以直接选择 `on` 的值或某个 arm，因此切换 `isOpen` 或列表数量会改变画面，而无需运行代码。

## 6. 样式

元素的 `style` 是一个声明列表。每条声明有 `property`、`value`、可选的 `when` 条件和 `from`（声明的来源）：`syntax`（`class`、`inline`、`modifier`、`stylesheet`、`theme`）、其 `text`（一个类、一个内联条目、一个 modifier）和 `location`。声明按顺序生效。在条件成立的声明中，后面的生效。

| 分组 | 属性 | 值 |
| --- | --- | --- |
| Layout | `direction` | `row`、`column`、`overlay` |
| | `alignItems` | `start`、`center`、`end`、`stretch`、`baseline` |
| | `justifyContent` | `start`、`center`、`end`、`spaceBetween`、`spaceAround`、`spaceEvenly` |
| | `wrap` | `wrap`、`nowrap` |
| | `grow` | number |
| Spacing | `gap`、`paddingTop`、`paddingBottom`、`paddingStart`、`paddingEnd` | length |
| | `marginTop`、`marginBottom`、`marginStart`、`marginEnd` | length、`auto` |
| Size | `width`、`height`、`minWidth`、`maxWidth`、`minHeight`、`maxHeight` | length、`auto`、`fill`、`fit` |
| Color | `color`、`background`、`borderColor` | color |
| | `opacity` | number |
| Typography | `fontFamily` | 任意 keyword |
| | `fontSize`、`letterSpacing` | length |
| | `fontWeight` | number |
| | `fontStyle` | `normal`、`italic` |
| | `lineHeight` | length 或 number |
| | `textAlign` | `start`、`center`、`end`、`justify` |
| Radius and border | `borderRadius`、`borderWidth` | length |

`Start` 和 `end` 是书写方向上的起始侧和结束侧。

| 值 `kind` | 字段 |
| --- | --- |
| `length` | `value` 和 `unit`：`px`、`rem`、`em`、`percent`、`pt`、`dp` |
| `color` | `value`：小写 sRGB `#rrggbb` 或 `#rrggbbaa` |
| `number` | `value` |
| `keyword` | 属性列表中的 `value` |
| `token` | `token` ID，解析后以 `resolved` 给出最终值 |
| `unresolved` | 源码 `text` 和 `reason`。所有属性都接受 |

当条件的所有字段都成立时条件成立：`minWidth` 和 `maxWidth`（length）、`interaction`（`hover`、`focus`、`pressed`、`disabled`）、`colorScheme`（`light`、`dark`），以及必须为真的表达式 `expr`。适配器看到但无法映射到属性的样式（例如 CSS Module 类）连同其 `syntax`、`text`、`reason` 和 `location` 放入 `unresolvedStyle`。浏览器永远不会填补它。

参考示例映射简单语法：Tailwind 类和内联样式。Tailwind 的 spacing 步长记录为解析后的长度。中等和困难语法（[样式与设计 token](roadmap.md#default-pipeline)）在其适配器出现后使用相同的声明。

## 7. Token

token 保留源码使用的名称（`--color-brand`、`--text-2xl`），以便之后成为设计变量。`group` 是 `color`、`spacing`、`typography`、`radius`、`size` 或 `other`。`value` 是最终值，或指向另一个 token 的 `token` 值（别名）。别名不得形成循环。`modes` 保存各模式的值，例如 `dark`。项目 token 有 `location`。框架默认值有 `package`（`tailwindcss`），没有位置。

## 8. 边

`kind` 是 `link`（声明式链接，例如 `<Link>` 或 `NavigationLink`）、`navigate`（命令式调用）或 `present`（覆盖在当前界面上的弹层或对话框）。`to` 是 `screen`、不匹配模型中任何界面的 `route` 路径，或 `opaque` 表达式。边从触发它的节点开始。其所属界面由绘制该节点的界面决定，包括经由组件实例的情况（`SourceModel::screen_edges`）。

## 9. 校验与版本

JSON Schema 检查形状、枚举、模式和必填字段。Rust 读取器（`SourceModel::from_json`）还会检查 schema 无法表达的规则：

- 标识符有序且唯一，槽位名称在组件内唯一；
- 每个位置和导入都指向已列出的文件；
- 每个引用都能解析：界面与实例的组件、实例 prop、同一组件内的槽位读取、列表内的列表项读取、列表项 schema、token，以及边的节点和界面；
- 样式值符合其属性，token 别名不循环，`otherwise` arm 位于最后。

`npm run check:source-model` 用 schema 校验参考示例，检查错误形状会被拒绝，并运行读取器的测试。这些测试还会检查参考示例中的每个位置都指向 fixture 源码，并且文件摘要一致。

版本 1 已冻结。对字段、种类或含义的任何改动都会产生带有新 schema 文件的 `schemaVersion` 2。版本 1 的文件和读取器不会改变。读取器拒绝它不认识的版本。
