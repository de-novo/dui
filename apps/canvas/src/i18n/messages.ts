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
  observedHeading: string;
  captureVerified: string;
  overview: string;
  backToOverview: string;
  backToArchive: string;
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
    observedHeading: "Observed heading",
    captureVerified: "Capture verified",
    overview: "Overview",
    backToOverview: "Back to overview",
    backToArchive: "Back to archive",
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
    observedHeading: "观测到的标题",
    captureVerified: "捕获已验证",
    overview: "概览",
    backToOverview: "返回概览",
    backToArchive: "返回归档",
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
    observedHeading: "관측한 제목",
    captureVerified: "캡처 검증됨",
    overview: "개요",
    backToOverview: "개요로 돌아가기",
    backToArchive: "아카이브로 돌아가기",
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
