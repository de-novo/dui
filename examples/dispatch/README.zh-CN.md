# Dispatch 提货看板

[English](README.md) · [한국어](README.ko.md) · [简体中文](README.zh-CN.md)

从提货看板进入提货详情，然后打开司机交接说明。示例使用 HTML、CSS 和原生对话框。 [观看录制演示（MP4，20 秒）](../recordings/dispatch.mp4)

## 捕获与元素检查

安装 dui、Node.js 和 Chromium 后，从仓库根目录开始。在临时副本中操作，使两次捕获属于同一项目历史。

```sh
work=$(mktemp -d)
cp -R examples/dispatch "$work/dispatch"
cd "$work/dispatch"
dui map . > "$work/before.json"
before=$(node -p 'require(process.argv[1]).output' "$work/before.json")
dui open "$before"
```

在地图上打开对话框卡片，查看捕获的交接说明。选择提货看板，在 **Element Inspector → Rendered elements** 中选择标题即可查看实测盒模型。

## 比较改动

按 Ctrl+C 关闭查看器。在同一终端修改标题并再次捕获。

```sh
node -e 'const fs = require("node:fs"); const p = "index.html"; fs.writeFileSync(p, fs.readFileSync(p, "utf8").replace("<h1>Pickup board</h1>", "<h1>Pickup board · revised</h1>"))'
dui map . > "$work/after.json"
after=$(node -p 'require(process.argv[1]).output' "$work/after.json")
dui diff "$before" "$after"
dui open "$before" "$after"
```

在 **Map options → Compare captures** 中选取修改前后的运行记录。CLI 会按捕获宽度报告标题文字变化。

## 在桌面端打开

关闭网页查看器，使用 GPUI 桌面构建打开首次运行记录。选择提货详情卡片即可查看相连的界面。

```sh
dui desktop --run "$(node -p 'require(process.argv[1]).snapshot.id' "$work/before.json")"
```

## 自动验证

安装 Rust、Node.js 26 和 Chromium 后，从仓库根目录运行以下命令。

```sh
npm ci
npm ci --prefix apps/canvas
npm run check:example
```

CI 会在每个捕获宽度下用 Chromium 点击链接并打开对话框，然后检查归档、Inspector 的选择结果和前后两次捕获的 Diff。要在本地验证原生渲染，请用 Cargo 构建 `apps/desktop/Cargo.toml` 并运行 `npm run check:example -- --desktop`。视频结尾的概览与详情快照由 macOS Metal 渲染。
