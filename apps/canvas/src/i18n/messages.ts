export type UiLocale = "en" | "zh-CN" | "ko";

export const UI_LOCALE_OPTIONS: readonly { value: UiLocale; label: string }[] = [
  { value: "en", label: "English" },
  { value: "zh-CN", label: "简体中文" },
  { value: "ko", label: "한국어" },
];

export function normalizeUiLocale(raw: string | null | undefined): UiLocale {
  if (!raw) return "en";
  const lower = raw.trim().toLowerCase();
  if (
    lower === "zh-cn" ||
    lower === "zh" ||
    lower.startsWith("zh-hans") ||
    lower.startsWith("zh_")
  ) {
    return "zh-CN";
  }
  if (lower === "ko" || lower.startsWith("ko-") || lower.startsWith("ko_")) {
    return "ko";
  }
  return "en";
}

export type Messages = {
  workspace: string;
  selectWorkspace: string;
  project: string;
  branch: string;
  branchNotRecorded: string;
  version: string;
  openWorkspace: string;
  showcaseProjects: string;
  homeLink: string;
  selectedScreenLabel: string;
  inspectScreen: string;
  zoomOneToOne: string;
  mobileViewMode: string;
  openModel: string;
  approximateModel: string;
  invalidModel: string;
  noSavedMaps: string;
  archiveLoadFailed: string;
  workspaceUnavailable: string;
  theme: string;
  themeSystem: string;
  themeLight: string;
  themeDark: string;
  language: string;
  expandRail: string;
  collapseRail: string;
  screenMap: string;
  findScreen: string;
  noMatchingScreens: string;
  miniMap: string;
  loadingMap: string;
  mapOptions: string;
  screenDetails: string;
  screenUnavailable: string;
  archive: string;
  screenEvidence: string;
  documentReady: string;
  projectReady: string;
  noScreenCondition: string;
  matched: string;
  unmatched: string;
  notObserved: string;
  visibility: string;
  didNotSettle: string;
  snapshotShown: (percent: string) => string;
  pngNoSnapshot: string;
  pngUnmeasured: string;
  pngFidelity: (percent: string) => string;
  pngZoomIn: string;
  snapshotOmitted: (count: number) => string;
  observedHeading: string;
  captureVerified: string;
  overview: string;
  backToOverview: string;
  backToArchive: string;
  responsivePreview: string;
  widthPreview: string;
  widthPreviewWidth: string;
  capturedWidths: string;
  widthCaptured: (width: number, percent: string) => string;
  widthReflowed: (from: number, to: number) => string;
  reflowAgreement: (pairs: string) => string;
  reflowUnmeasured: string;
  reflowWarning: string;
  widthLive: (width: number) => string;
  liveRendering: string;
  liveRender: string;
  liveRenderFailed: string;
  liveCardShown: string;
  widthLiveFailed: (reason: string) => string;
  viewport: string;
  constraint: string;
  height: string;
  applyLayout: string;
  openLive: string;
  closeLiveSession: string;
  liveControls: string;
  livePreview: string;
  starting: string;
  disconnected: string;
  startingLiveSession: string;
  liveUnavailable: string;
  reopen: string;
  cancel: string;
  loadingLivePreview: string;
  loadingStates: string;
  frameFailed: string;
  noEditableStates: string;
  readOnlyCount: (count: number) => string;
  notShown: string;
  reset: string;
  on: string;
  off: string;
  saveSnapshot: string;
  savedSnapshot: string;
  openSaved: string;
  couldNotApply: string;
  valueDidNotStick: string;
  width: string;
  widths: string;
  statesAndWidths: string;
  stateLabel: (index: number) => string;
  localeFilter: string;
  allLocales: string;
  localeIssues: string;
  missingKeysLabel: (keys: string[]) => string;
  overflowLabel: (selector: string) => string;
  rtlMismatchLabel: (observed: string) => string;
  noImage: string;
  imageUnavailable: string;
  stateUnavailable: string;
  noCaptureAtWidth: string;
  verified: string;
  unverified: string;
  sourcePreview: string;
  sourcePreviewNotNative: string;
  previewSwitch: (number: number) => string;
  previewStatus: Record<"missing" | "stale" | "unsupported", string>;
  sourceDeclaration: string;
  dynamicRoute: string;
  discoveryLimitations: string;
  noDeclaredScreens: string;
  routeCondition: string;
  unrenderable: string;
  links: string;
  discovered: string;
  linkEvidence: string;
  domAnchor: string;
  observedAnchor: string;
  notReplayVerified: string;
  edgeExtracted: string;
  edgeInferred: string;
  edgeVerified: string;
  replayVerifiedLabel: string;
  compareCaptures: string;
  diffTitle: string;
  beforeCapture: string;
  afterCapture: string;
  loadingDiff: string;
  diffFailed: string;
  emptyDiffTitle: string;
  emptyDiffDetail: (unchanged: number) => string;
  diffSummaryLabel: (changed: number, unchanged: number) => string;
  diffAdded: string;
  diffRemoved: string;
  diffChanged: string;
  beforeLabel: string;
  afterLabel: string;
  pixelDiffOverlay: string;
  changedFilesLabel: string;
  statusChangeLabel: string;
  pixelsChangedLabel: (pixels: number, percent: string) => string;
  comments: string;
  commentTarget: string;
  commentTargetScreen: string;
  commentTargetWidth: string;
  commentTargetTransition: string;
  commentPlaceholder: string;
  addComment: string;
  resolveComment: string;
  reopenComment: string;
  commentOpen: string;
  commentResolved: string;
  noComments: string;
  commentFailed: string;
  auditOpenCommentsLabel: (count: number) => string;
  selectMapScreen: string;
  mapConnections: string;
  allMapLinks: string;
  mapEdgeStart: string;
  edgeStartElement: string;
  edgeStartCard: string;
  screenMapLinks: string;
  mapCaptureWidth: string;
  allMapCards: string;
  elementFrames: string;
  alignmentGuides: string;
  showGuide: string;
  selectedBounds: string;
  alignmentChanges: (count: number) => string;
  alignmentSummary: (count: number, narrower: number, wider: number) => string;
  alignmentUnverified: string;
  guideEdges: Record<"middle" | "bottom" | "top" | "center" | "left" | "right", string>;
  elementInspectorTitle: string;
  boxModelLabel: string;
  elementAttributesLabel: string;
  elementTreeLabel: string;
  canvasDiffCompare: string;
  canvasDiffOff: string;
  canvasDiffChangedOnly: string;
  canvasDiffModeRender: string;
  canvasDiffModeAfter: string;
  canvasDiffModeBefore: string;
  canvasDiffModeOverlay: string;
  canvasDiffElementsCount: (count: number) => string;
  canvasDiffVisualOnly: string;
  canvasDiffSummary: (changed: number, added: number, removed: number) => string;
  canvasDiffNoChanges: string;
  mobileViewCards: string;
  mobileViewMap: string;
};

export const MESSAGES: Record<UiLocale, Messages> = {
  en: {
    workspace: "Workspace",
    selectWorkspace: "Select workspace",
    project: "Project",
    branch: "Branch",
    branchNotRecorded: "Branch not recorded",
    version: "Version",
    openWorkspace: "Open workspace",
    showcaseProjects: "Showcase projects",
    homeLink: "Back to dui.sh",
    selectedScreenLabel: "Selected",
    inspectScreen: "Inspect screen",
    zoomOneToOne: "Zoom to 1:1 actual capture size",
    mobileViewMode: "Mobile view mode",
    openModel: "Open source model",
    approximateModel: "Approximate model",
    invalidModel: "Could not read this schema-v1 model.",
    noSavedMaps: "No saved screen maps.",
    archiveLoadFailed: "Could not load the archive.",
    workspaceUnavailable: "This workspace is unavailable.",
    theme: "Theme",
    themeSystem: "System theme",
    themeLight: "Light theme",
    themeDark: "Dark theme",
    language: "Language",
    expandRail: "Expand rail",
    collapseRail: "Collapse rail",
    screenMap: "Screen map",
    findScreen: "Find screen",
    noMatchingScreens: "No matching screens",
    miniMap: "Minimap",
    loadingMap: "Loading…",
    mapOptions: "Map options",
    screenDetails: "Screen details",
    screenUnavailable: "Screen unavailable",
    archive: "Archive",
    screenEvidence: "Screen verification",
    documentReady: "Default readiness check",
    projectReady: "Project condition",
    noScreenCondition: "No project screen condition configured",
    matched: "Matched",
    unmatched: "Not matched",
    notObserved: "Not observed",
    visibility: "Visible match",
    didNotSettle: "Page did not settle",
    snapshotShown: (percent) =>
      `Card shows the HTML snapshot · ${percent} of pixels differ from the PNG`,
    pngNoSnapshot: "Card shows the PNG · no HTML snapshot",
    pngUnmeasured: "Card shows the PNG · the HTML snapshot was not compared",
    pngFidelity: (percent) =>
      `Card shows the PNG · the HTML snapshot differs in ${percent} of pixels`,
    pngZoomIn: "Card shows the PNG · zoom in on it to see the HTML snapshot",
    snapshotOmitted: (count) => `${count} resource${count === 1 ? "" : "s"} not in the snapshot`,
    observedHeading: "Observed heading",
    captureVerified: "Capture verified",
    overview: "Overview",
    backToOverview: "Back to overview",
    backToArchive: "Back to archive",
    responsivePreview: "Responsive preview",
    widthPreview: "Width preview",
    widthPreviewWidth: "Width (CSS px)",
    capturedWidths: "Captured widths",
    widthCaptured: (width, percent) =>
      `Captured at ${width} px: its HTML snapshot, ${percent} of pixels differ from the PNG`,
    widthReflowed: (from, to) =>
      `CSS reflow preview, not verified: the ${from} px snapshot laid out at ${to} px`,
    reflowAgreement: (pairs) =>
      `This snapshot at the other captured widths, against their PNGs: ${pairs}`,
    reflowUnmeasured: "Only one captured width, so reflow agreement is not measured.",
    widthLive: (width) =>
      `Rendered from source at ${width} px in the running app, scripts included. Not recorded as a capture.`,
    liveRendering: "Rendering from source…",
    liveRender: "Render from source",
    liveRenderFailed: "Could not render from source",
    liveCardShown:
      "The card shows the screen rendered from source in the running app; it is not a capture",
    widthLiveFailed: (reason) =>
      `The running app could not be opened (${reason}). Showing the HTML snapshot instead.`,
    reflowWarning:
      "Scripts also change this screen's layout with its width, so other widths may differ from the app. Use Responsive preview on a running app to see them.",
    viewport: "Viewport",
    constraint: "Resize target",
    height: "Height",
    applyLayout: "Apply",
    openLive: "Open live",
    closeLiveSession: "Close live session",
    liveControls: "Live state controls",
    livePreview: "Live screen preview",
    starting: "Starting…",
    disconnected: "Disconnected",
    startingLiveSession: "Starting live session…",
    liveUnavailable: "Live session unavailable",
    reopen: "Reopen",
    cancel: "Cancel",
    loadingLivePreview: "Loading live preview…",
    loadingStates: "Loading states…",
    frameFailed: "Could not capture the live frame.",
    noEditableStates: "No editable states on this screen.",
    readOnlyCount: (count) => `${count} read-only`,
    notShown: "Not shown",
    reset: "Reset",
    on: "On",
    off: "Off",
    saveSnapshot: "Save snapshot",
    savedSnapshot: "Saved snapshot",
    openSaved: "Open saved",
    couldNotApply: "Could not apply change",
    valueDidNotStick: "Value did not stick",
    width: "Width",
    widths: "Widths",
    statesAndWidths: "States & widths",
    stateLabel: (index) => `State ${index}`,
    localeFilter: "Locale",
    allLocales: "All locales",
    localeIssues: "Locale issues",
    missingKeysLabel: (keys) => `Missing keys: ${keys.join(", ")}`,
    overflowLabel: (selector) => `Overflow: ${selector}`,
    rtlMismatchLabel: (observed) => `Expected RTL, got ${observed}`,
    noImage: "No image",
    imageUnavailable: "Image unavailable",
    stateUnavailable: "State unavailable",
    noCaptureAtWidth: "No capture at this width",
    verified: "Verified",
    unverified: "Unverified",
    sourcePreview: "Source preview",
    sourcePreviewNotNative:
      "Redrawn in HTML from portable layout primitives. Not the native iOS or Android screen.",
    previewSwitch: (number) => "Switch " + number,
    previewStatus: {
      missing: "Source missing",
      stale: "Source changed",
      unsupported: "Preview unavailable",
    },
    sourceDeclaration: "Source declaration",
    dynamicRoute: "Dynamic route",
    discoveryLimitations: "Discovery limitations",
    noDeclaredScreens: "No screen declarations found",
    routeCondition: "Condition",
    unrenderable: "Unrenderable",
    links: "Links",
    discovered: "Discovered",
    linkEvidence: "Link evidence",
    domAnchor: "DOM anchor",
    observedAnchor: "Observed anchor",
    notReplayVerified: "Not replay-verified",
    edgeExtracted: "Extracted",
    edgeInferred: "Inferred",
    edgeVerified: "Verified by replay",
    replayVerifiedLabel: "Replay-verified",
    compareCaptures: "Compare captures",
    diffTitle: "Capture diff",
    beforeCapture: "Before capture",
    afterCapture: "After capture",
    loadingDiff: "Comparing captures…",
    diffFailed: "Could not compare these captures.",
    emptyDiffTitle: "No changed cells",
    emptyDiffDetail: (unchanged) => `All ${unchanged} cells match between these captures.`,
    diffSummaryLabel: (changed, unchanged) => `${changed} changed · ${unchanged} unchanged hidden`,
    diffAdded: "Added",
    diffRemoved: "Removed",
    diffChanged: "Changed",
    beforeLabel: "Before",
    afterLabel: "After",
    pixelDiffOverlay: "Pixel difference overlay",
    changedFilesLabel: "Changed source files",
    statusChangeLabel: "Status change",
    pixelsChangedLabel: (pixels, percent) => `${pixels} px changed (${percent}%)`,
    comments: "Review comments",
    commentTarget: "Attach to",
    commentTargetScreen: "Screen",
    commentTargetWidth: "Width",
    commentTargetTransition: "Transition",
    commentPlaceholder: "Leave review feedback…",
    addComment: "Post",
    resolveComment: "Resolve",
    reopenComment: "Reopen",
    commentOpen: "Open",
    commentResolved: "Resolved",
    noComments: "No review comments on this screen yet.",
    commentFailed: "Could not save review comments.",
    auditOpenCommentsLabel: (count) => `${count} unresolved`,
    selectMapScreen: "Select a screen",
    mapConnections: "Connections",
    allMapLinks: "All links",
    mapEdgeStart: "Links start at",
    edgeStartElement: "Link element",
    edgeStartCard: "Card border",
    screenMapLinks: "Links to and from a screen",
    mapCaptureWidth: "Capture width",
    allMapCards: "All map cards",
    elementFrames: "Element frames",
    alignmentGuides: "Alignment guides",
    showGuide: "Show guide",
    selectedBounds: "Selected element edges",
    alignmentChanges: (n) => `${n} alignment changes`,
    alignmentSummary: (n, a, b) =>
      `${n} alignment ${n === 1 ? "change" : "changes"} · Compared with ${a} and ${b} px`,
    alignmentUnverified:
      "Automatic comparison needs matching, aligned controls at a narrower and a wider capture width.",
    guideEdges: {
      middle: "Row center",
      top: "Top",
      bottom: "Bottom",
      center: "Column center",
      left: "Left",
      right: "Right",
    },
    elementInspectorTitle: "Element inspector",
    boxModelLabel: "Box model",
    elementAttributesLabel: "Attributes",
    elementTreeLabel: "Rendered elements",
    canvasDiffCompare: "Compare",
    canvasDiffOff: "No diff",
    canvasDiffChangedOnly: "Changed only",
    canvasDiffModeRender: "Elements",
    canvasDiffModeAfter: "After",
    canvasDiffModeBefore: "Before",
    canvasDiffModeOverlay: "Pixel",
    canvasDiffElementsCount: (count) => `${count} el`,
    canvasDiffVisualOnly: "Visual-only change",
    canvasDiffSummary: (changed, added, removed) =>
      `${changed} changed · ${added} added · ${removed} removed`,
    canvasDiffNoChanges: "No UI changes",
    mobileViewCards: "Cards",
    mobileViewMap: "2D Map",
  },
  "zh-CN": {
    workspace: "工作区",
    selectWorkspace: "选择工作区",
    project: "项目",
    branch: "分支",
    branchNotRecorded: "未记录分支",
    version: "版本",
    openWorkspace: "打开工作区",
    showcaseProjects: "演示项目",
    homeLink: "返回 dui.sh",
    selectedScreenLabel: "已选择",
    inspectScreen: "检查界面",
    zoomOneToOne: "缩放到 1:1 实际捕获尺寸",
    mobileViewMode: "移动端视图模式",
    openModel: "打开源模型",
    approximateModel: "近似模型",
    invalidModel: "无法读取此版本 1 模型。",
    noSavedMaps: "暂无已保存的界面地图。",
    archiveLoadFailed: "无法加载归档数据。",
    workspaceUnavailable: "该工作区不可用。",
    theme: "主题",
    themeSystem: "跟随系统",
    themeLight: "浅色主题",
    themeDark: "深色主题",
    language: "语言",
    expandRail: "展开侧栏",
    collapseRail: "收起侧栏",
    screenMap: "界面地图",
    findScreen: "查找屏幕",
    noMatchingScreens: "没有匹配的屏幕",
    miniMap: "缩略地图",
    loadingMap: "加载中…",
    mapOptions: "地图选项",
    screenDetails: "界面详情",
    screenUnavailable: "界面不可用",
    archive: "归档",
    screenEvidence: "页面验证",
    documentReady: "默认就绪检查",
    projectReady: "项目条件",
    noScreenCondition: "未配置项目画面条件",
    matched: "匹配",
    unmatched: "不匹配",
    notObserved: "未观测",
    visibility: "可见匹配",
    didNotSettle: "页面未稳定",
    snapshotShown: (percent) => `卡片显示 HTML 快照 · 与 PNG 不同的像素占 ${percent}`,
    pngNoSnapshot: "卡片显示 PNG · 没有 HTML 快照",
    pngUnmeasured: "卡片显示 PNG · HTML 快照未经比较",
    pngFidelity: (percent) => `卡片显示 PNG · HTML 快照有 ${percent} 的像素不同`,
    pngZoomIn: "卡片显示 PNG · 放大该卡片即可查看 HTML 快照",
    snapshotOmitted: (count) => `${count} 个资源未包含在快照中`,
    observedHeading: "观测到的标题",
    captureVerified: "捕获已验证",
    overview: "概览",
    backToOverview: "返回概览",
    backToArchive: "返回归档",
    responsivePreview: "响应式预览",
    widthPreview: "宽度预览",
    widthPreviewWidth: "宽度（CSS px）",
    capturedWidths: "已捕获宽度",
    widthCaptured: (width, percent) =>
      `已在 ${width} px 捕获：其 HTML 快照，与 PNG 不同的像素占 ${percent}`,
    widthReflowed: (from, to) => `CSS 重排预览，未验证：将 ${from} px 快照按 ${to} px 排版`,
    reflowAgreement: (pairs) => `此快照按其他已捕获宽度排版，与其 PNG 对比：${pairs}`,
    reflowUnmeasured: "只有一个已捕获宽度，无法测量重排一致度。",
    widthLive: (width) => `在运行中的应用里按 ${width} px 从源码渲染，包含脚本。不记录为捕获。`,
    liveRendering: "正在从源码渲染…",
    liveRender: "从源码渲染",
    liveRenderFailed: "无法从源码渲染",
    liveCardShown: "卡片显示在运行中的应用里从源码渲染的界面，不是捕获",
    widthLiveFailed: (reason) => `无法打开运行中的应用（${reason}），改为显示 HTML 快照。`,
    reflowWarning:
      "脚本也会随宽度改变此界面的布局，因此其他宽度可能与应用不同。可在运行中的应用上使用响应式预览查看。",
    viewport: "视口",
    constraint: "调整大小的对象",
    height: "高度",
    applyLayout: "应用",
    openLive: "打开实时会话",
    closeLiveSession: "关闭实时会话",
    liveControls: "实时状态控制",
    livePreview: "实时界面预览",
    starting: "正在启动…",
    disconnected: "已断开",
    startingLiveSession: "正在启动实时会话…",
    liveUnavailable: "实时会话不可用",
    reopen: "重新打开",
    cancel: "取消",
    loadingLivePreview: "正在加载实时画面…",
    loadingStates: "正在加载状态…",
    frameFailed: "无法捕获实时画面。",
    noEditableStates: "此界面没有可编辑状态。",
    readOnlyCount: (count) => `${count} 项只读`,
    notShown: "未显示",
    reset: "重置",
    on: "开",
    off: "关",
    saveSnapshot: "保存快照",
    savedSnapshot: "已保存快照",
    openSaved: "打开已保存快照",
    couldNotApply: "无法应用更改",
    valueDidNotStick: "状态值未生效",
    width: "宽度",
    widths: "宽度",
    statesAndWidths: "状态与宽度",
    stateLabel: (index) => `状态 ${index}`,
    localeFilter: "语言区域",
    allLocales: "全部语言",
    localeIssues: "多语言问题",
    missingKeysLabel: (keys) => `缺失翻译键：${keys.join(", ")}`,
    overflowLabel: (selector) => `文本溢出：${selector}`,
    rtlMismatchLabel: (observed) => `预期 RTL，实际为 ${observed}`,
    noImage: "无图像",
    imageUnavailable: "图像不可用",
    stateUnavailable: "状态不可用",
    noCaptureAtWidth: "此宽度暂无截图",
    verified: "已验证",
    unverified: "未验证",
    sourcePreview: "源码预览",
    sourcePreviewNotNative: "依据可移植布局元素以 HTML 重新绘制，并非真实的 iOS 或 Android 界面。",
    previewSwitch: (number) => "开关 " + number,
    previewStatus: { missing: "源文件缺失", stale: "源码已更改", unsupported: "无法预览" },
    sourceDeclaration: "源码声明",
    dynamicRoute: "动态路由",
    discoveryLimitations: "识别限制",
    noDeclaredScreens: "未发现屏幕声明",
    routeCondition: "条件",
    unrenderable: "无法渲染",
    links: "链接",
    discovered: "已发现",
    linkEvidence: "链接依据",
    domAnchor: "DOM 链接",
    observedAnchor: "观测到的链接",
    notReplayVerified: "未经回放验证",
    edgeExtracted: "代码提取",
    edgeInferred: "推断",
    edgeVerified: "回放已验证",
    replayVerifiedLabel: "回放已验证",
    compareCaptures: "对比两次捕获",
    diffTitle: "捕获差异",
    beforeCapture: "变更前捕获",
    afterCapture: "变更后捕获",
    loadingDiff: "正在对比捕获…",
    diffFailed: "无法对比这两次捕获。",
    emptyDiffTitle: "无变更单元格",
    emptyDiffDetail: (unchanged) => `两次捕获间全部 ${unchanged} 个单元格一致。`,
    diffSummaryLabel: (changed, unchanged) => `${changed} 处变更 · 已隐藏 ${unchanged} 个未变更项`,
    diffAdded: "新增",
    diffRemoved: "移除",
    diffChanged: "已变更",
    beforeLabel: "变更前",
    afterLabel: "变更后",
    pixelDiffOverlay: "像素差异叠加层",
    changedFilesLabel: "变更源文件",
    statusChangeLabel: "状态变化",
    pixelsChangedLabel: (pixels, percent) => `变化 ${pixels} 像素 (${percent}%)`,
    comments: "评审评论",
    commentTarget: "附着位置",
    commentTargetScreen: "界面",
    commentTargetWidth: "宽度",
    commentTargetTransition: "跳转",
    commentPlaceholder: "输入评审意见…",
    addComment: "提交",
    resolveComment: "解决",
    reopenComment: "重新打开",
    commentOpen: "待处理",
    commentResolved: "已解决",
    noComments: "此界面暂无评审评论。",
    commentFailed: "无法保存评审评论。",
    auditOpenCommentsLabel: (count) => `${count} 条待处理评论`,
    selectMapScreen: "选择屏幕",
    mapConnections: "连接",
    allMapLinks: "所有连接",
    mapEdgeStart: "连接起点",
    edgeStartElement: "链接元素",
    edgeStartCard: "卡片边框",
    screenMapLinks: "与屏幕关联的连接",
    mapCaptureWidth: "捕获宽度",
    allMapCards: "全部地图卡片",
    elementFrames: "元素边框",
    alignmentGuides: "对齐参考线",
    showGuide: "显示参考线",
    selectedBounds: "所选元素边界",
    alignmentChanges: (n) => `${n} 处对齐变化`,
    alignmentSummary: (n, a, b) => `与 ${a} 和 ${b} px 的截图相比，有 ${n} 处对齐变化`,
    alignmentUnverified: "自动比较需要在更窄和更宽的截图中找到彼此对齐的相同控件。",
    guideEdges: {
      middle: "行中心",
      top: "顶部",
      bottom: "底部",
      center: "列中心",
      left: "左侧",
      right: "右侧",
    },
    elementInspectorTitle: "元素检查器",
    boxModelLabel: "盒模型",
    elementAttributesLabel: "元素属性",
    elementTreeLabel: "已渲染元素",
    canvasDiffCompare: "对比",
    canvasDiffOff: "关闭对比",
    canvasDiffChangedOnly: "仅看变更界面",
    canvasDiffModeRender: "元素",
    canvasDiffModeAfter: "变更后",
    canvasDiffModeBefore: "变更前",
    canvasDiffModeOverlay: "像素",
    canvasDiffElementsCount: (count) => `${count} 处元素`,
    canvasDiffVisualOnly: "仅视觉变更",
    canvasDiffSummary: (changed, added, removed) =>
      `变更 ${changed} · 新增 ${added} · 删除 ${removed}`,
    canvasDiffNoChanges: "无界面变更",
    mobileViewCards: "卡片",
    mobileViewMap: "2D 地图",
  },
  ko: {
    workspace: "워크스페이스",
    selectWorkspace: "워크스페이스 선택",
    project: "프로젝트",
    branch: "브랜치",
    branchNotRecorded: "브랜치 미기록",
    version: "버전",
    openWorkspace: "워크스페이스 열기",
    showcaseProjects: "쇼케이스 프로젝트",
    homeLink: "dui.sh로 돌아가기",
    selectedScreenLabel: "선택됨",
    inspectScreen: "화면 검사",
    zoomOneToOne: "1:1 실제 캡처 크기로 확대",
    mobileViewMode: "모바일 보기 모드",
    openModel: "소스 모델 열기",
    approximateModel: "근사 모델",
    invalidModel: "스키마 v1 모델을 읽을 수 없습니다.",
    noSavedMaps: "저장된 화면 지도가 없습니다.",
    archiveLoadFailed: "아카이브를 불러올 수 없습니다.",
    workspaceUnavailable: "이 워크스페이스를 사용할 수 없습니다.",
    theme: "테마",
    themeSystem: "시스템 테마",
    themeLight: "라이트 테마",
    themeDark: "다크 테마",
    language: "언어",
    expandRail: "사이드바 펼치기",
    collapseRail: "사이드바 접기",
    screenMap: "화면 지도",
    findScreen: "화면 찾기",
    noMatchingScreens: "일치하는 화면이 없습니다",
    miniMap: "미니맵",
    loadingMap: "불러오는 중…",
    mapOptions: "지도 옵션",
    screenDetails: "화면 상세",
    screenUnavailable: "화면 없음",
    archive: "아카이브",
    screenEvidence: "화면 검증",
    documentReady: "기본 준비 조건",
    projectReady: "프로젝트 조건",
    noScreenCondition: "프로젝트 화면 조건 미지정",
    matched: "일치",
    unmatched: "불일치",
    notObserved: "관측되지 않음",
    visibility: "보이는 요소",
    didNotSettle: "페이지 안정화 미확인",
    snapshotShown: (percent) => `카드가 HTML 스냅샷을 표시 · PNG와 다른 픽셀 ${percent}`,
    pngNoSnapshot: "카드가 PNG를 표시 · HTML 스냅샷 없음",
    pngUnmeasured: "카드가 PNG를 표시 · HTML 스냅샷을 비교하지 않음",
    pngFidelity: (percent) => `카드가 PNG를 표시 · HTML 스냅샷이 픽셀 ${percent}에서 다름`,
    pngZoomIn: "카드가 PNG를 표시 · 카드를 확대하면 HTML 스냅샷이 보임",
    snapshotOmitted: (count) => `스냅샷에 넣지 못한 리소스 ${count}개`,
    observedHeading: "관측한 제목",
    captureVerified: "캡처 검증됨",
    overview: "개요",
    backToOverview: "개요로 돌아가기",
    backToArchive: "아카이브로 돌아가기",
    responsivePreview: "반응형 미리보기",
    widthPreview: "폭 미리보기",
    widthPreviewWidth: "폭(CSS px)",
    capturedWidths: "캡처한 폭",
    widthCaptured: (width, percent) =>
      `${width} px에서 캡처함: 이 폭의 HTML 스냅샷, PNG와 다른 픽셀 ${percent}`,
    widthReflowed: (from, to) =>
      `CSS 재배치 미리보기 · 미검증: ${from} px 스냅샷을 ${to} px로 다시 그림`,
    reflowAgreement: (pairs) => `이 스냅샷을 다른 캡처 폭으로 다시 그려 PNG와 비교: ${pairs}`,
    reflowUnmeasured: "캡처한 폭이 하나뿐이라 재배치 일치도를 측정하지 못했습니다.",
    widthLive: (width) =>
      `실행 중인 앱에서 소스를 ${width} px로 렌더링함 (스크립트 포함). 캡처로 기록되지 않음.`,
    liveRendering: "소스에서 렌더링 중…",
    liveRender: "소스에서 렌더링",
    liveRenderFailed: "소스에서 렌더링하지 못함",
    liveCardShown: "카드가 실행 중인 앱에서 소스로 렌더링한 화면을 표시 · 캡처 아님",
    widthLiveFailed: (reason) =>
      `실행 중인 앱을 열지 못했습니다 (${reason}). 대신 HTML 스냅샷을 보여 줍니다.`,
    reflowWarning:
      "이 화면은 스크립트도 폭에 따라 레이아웃을 바꾸므로, 다른 폭은 실제 앱과 다를 수 있습니다. 실행 중인 앱에서는 반응형 미리보기로 확인하세요.",
    viewport: "화면 영역",
    constraint: "크기 조절 대상",
    height: "높이",
    applyLayout: "적용",
    openLive: "라이브 열기",
    closeLiveSession: "라이브 세션 닫기",
    liveControls: "라이브 상태 조작",
    livePreview: "라이브 화면 미리보기",
    starting: "시작 중…",
    disconnected: "연결 끊김",
    startingLiveSession: "라이브 세션 시작 중…",
    liveUnavailable: "라이브 세션 사용 불가",
    reopen: "다시 열기",
    cancel: "취소",
    loadingLivePreview: "라이브 화면 불러오는 중…",
    loadingStates: "상태 불러오는 중…",
    frameFailed: "라이브 프레임을 캡처할 수 없습니다.",
    noEditableStates: "이 화면에 편집할 상태가 없습니다.",
    readOnlyCount: (count) => `읽기 전용 ${count}개`,
    notShown: "표시 안 됨",
    reset: "초기화",
    on: "켜짐",
    off: "꺼짐",
    saveSnapshot: "스냅샷 저장",
    savedSnapshot: "스냅샷 저장됨",
    openSaved: "저장본 열기",
    couldNotApply: "변경을 적용할 수 없습니다",
    valueDidNotStick: "값이 유지되지 않았습니다",
    width: "폭",
    widths: "폭",
    statesAndWidths: "상태 및 폭",
    stateLabel: (index) => `상태 ${index}`,
    localeFilter: "로케일",
    allLocales: "전체 언어",
    localeIssues: "다국어 점검",
    missingKeysLabel: (keys) => `누락 번역 키: ${keys.join(", ")}`,
    overflowLabel: (selector) => `텍스트 넘침: ${selector}`,
    rtlMismatchLabel: (observed) => `RTL 예상, 실제 ${observed}`,
    noImage: "이미지 없음",
    imageUnavailable: "이미지 없음",
    stateUnavailable: "상태 없음",
    noCaptureAtWidth: "이 폭의 캡처가 없습니다",
    verified: "검증됨",
    unverified: "미검증",
    sourcePreview: "소스 미리보기",
    sourcePreviewNotNative:
      "이식 가능한 레이아웃 요소를 HTML로 다시 그린 것입니다. 실제 iOS·Android 화면이 아닙니다.",
    previewSwitch: (number) => "스위치 " + number,
    previewStatus: { missing: "소스 없음", stale: "소스 변경됨", unsupported: "미리보기 불가" },
    sourceDeclaration: "소스 선언",
    dynamicRoute: "동적 경로",
    discoveryLimitations: "화면 탐색의 제약",
    noDeclaredScreens: "화면 선언을 찾지 못했습니다",
    routeCondition: "조건",
    unrenderable: "렌더 불가",
    links: "링크",
    discovered: "발견됨",
    linkEvidence: "링크 근거",
    domAnchor: "DOM 앵커",
    observedAnchor: "관측된 앵커",
    notReplayVerified: "재생 미검증",
    edgeExtracted: "코드 추출",
    edgeInferred: "추정",
    edgeVerified: "재생 검증됨",
    replayVerifiedLabel: "재생 검증됨",
    compareCaptures: "캡처 비교",
    diffTitle: "캡처 변경 비교",
    beforeCapture: "이전 캡처",
    afterCapture: "이후 캡처",
    loadingDiff: "캡처 비교 중…",
    diffFailed: "두 캡처를 비교할 수 없습니다.",
    emptyDiffTitle: "변경된 칸 없음",
    emptyDiffDetail: (unchanged) => `두 캡처의 ${unchanged}개 칸이 모두 동일합니다.`,
    diffSummaryLabel: (changed, unchanged) => `변경 ${changed}개 · 동일 ${unchanged}개 숨김`,
    diffAdded: "추가됨",
    diffRemoved: "삭제됨",
    diffChanged: "변경됨",
    beforeLabel: "이전",
    afterLabel: "이후",
    pixelDiffOverlay: "픽셀 차이 오버레이",
    changedFilesLabel: "변경된 소스 파일",
    statusChangeLabel: "상태 변경",
    pixelsChangedLabel: (pixels, percent) => `${pixels}px 변경 (${percent}%)`,
    comments: "리뷰 코멘트",
    commentTarget: "위치",
    commentTargetScreen: "화면",
    commentTargetWidth: "폭",
    commentTargetTransition: "전환",
    commentPlaceholder: "리뷰 의견을 입력하세요…",
    addComment: "등록",
    resolveComment: "해결",
    reopenComment: "다시 열기",
    commentOpen: "미해결",
    commentResolved: "해결됨",
    noComments: "이 화면에 등록된 코멘트가 없습니다.",
    commentFailed: "코멘트를 불러오거나 저장할 수 없습니다.",
    auditOpenCommentsLabel: (count) => `미해결 코멘트 ${count}개`,
    selectMapScreen: "화면 선택",
    mapConnections: "연결선",
    allMapLinks: "모든 연결선",
    mapEdgeStart: "연결선 시작점",
    edgeStartElement: "링크 요소",
    edgeStartCard: "카드 테두리",
    screenMapLinks: "화면에 연결된 경로",
    mapCaptureWidth: "캡처 폭",
    allMapCards: "지도 전체 카드",
    elementFrames: "요소 프레임",
    alignmentGuides: "정렬 가이드",
    showGuide: "표시할 가이드",
    selectedBounds: "선택한 요소의 경계",
    alignmentChanges: (n) => `정렬 변화 ${n}건`,
    alignmentSummary: (n, a, b) => `${a}px·${b}px와 비교한 정렬 변화 ${n}건`,
    alignmentUnverified:
      "자동 비교에는 더 좁은 캡처와 더 넓은 캡처에서 같은 선에 놓인 조작 요소가 필요합니다.",
    guideEdges: {
      middle: "행 중심",
      top: "상단",
      bottom: "하단",
      center: "열 중심",
      left: "왼쪽",
      right: "오른쪽",
    },
    elementInspectorTitle: "요소 인스펙터",
    boxModelLabel: "박스 모델",
    elementAttributesLabel: "요소 속성",
    elementTreeLabel: "렌더된 요소",
    canvasDiffCompare: "비교",
    canvasDiffOff: "비교 끄기",
    canvasDiffChangedOnly: "변경된 화면만",
    canvasDiffModeRender: "요소",
    canvasDiffModeAfter: "이후",
    canvasDiffModeBefore: "이전",
    canvasDiffModeOverlay: "픽셀",
    canvasDiffElementsCount: (count) => `${count}요소`,
    canvasDiffVisualOnly: "시각 전용 변경",
    canvasDiffSummary: (changed, added, removed) =>
      `변경 ${changed} · 추가 ${added} · 삭제 ${removed}`,
    canvasDiffNoChanges: "UI 변경 없음",
    mobileViewCards: "카드",
    mobileViewMap: "2D 맵",
  },
};
