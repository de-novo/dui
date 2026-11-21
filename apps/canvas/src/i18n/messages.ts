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
  theme: string;
  themeSystem: string;
  themeLight: string;
  themeDark: string;
  language: string;
  expandRail: string;
  collapseRail: string;
  screenMap: string;
  screenDetails: string;
  screenUnavailable: string;
  archive: string;
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
};

export const MESSAGES: Record<UiLocale, Messages> = {
  en: {
    workspace: "Workspace",
    selectWorkspace: "Select workspace",
    theme: "Theme",
    themeSystem: "System theme",
    themeLight: "Light theme",
    themeDark: "Dark theme",
    language: "Language",
    expandRail: "Expand rail",
    collapseRail: "Collapse rail",
    screenMap: "Screen map",
    screenDetails: "Screen details",
    screenUnavailable: "Screen unavailable",
    archive: "Archive",
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
  },
  "zh-CN": {
    workspace: "工作区",
    selectWorkspace: "选择工作区",
    theme: "主题",
    themeSystem: "跟随系统",
    themeLight: "浅色主题",
    themeDark: "深色主题",
    language: "语言",
    expandRail: "展开侧栏",
    collapseRail: "收起侧栏",
    screenMap: "界面地图",
    screenDetails: "界面详情",
    screenUnavailable: "界面不可用",
    archive: "归档",
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
  },
  ko: {
    workspace: "워크스페이스",
    selectWorkspace: "워크스페이스 선택",
    theme: "테마",
    themeSystem: "시스템 테마",
    themeLight: "라이트 테마",
    themeDark: "다크 테마",
    language: "언어",
    expandRail: "사이드바 펼치기",
    collapseRail: "사이드바 접기",
    screenMap: "화면 지도",
    screenDetails: "화면 상세",
    screenUnavailable: "화면 없음",
    archive: "아카이브",
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
  },
};
