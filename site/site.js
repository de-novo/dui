(() => {
  const root = document.documentElement;
  const savedTheme = localStorage.getItem("dui-site-theme");
  if (savedTheme === "light" || savedTheme === "dark") {
    root.dataset.theme = savedTheme;
  }

  const themeBtn = document.querySelector("[data-theme-toggle]");
  if (themeBtn) {
    const syncThemeLabel = () => {
      const isLight = root.dataset.theme === "light";
      themeBtn.textContent = isLight ? "Dark" : "Light";
      themeBtn.setAttribute("aria-pressed", String(isLight));
    };
    syncThemeLabel();
    themeBtn.addEventListener("click", () => {
      const next = root.dataset.theme === "light" ? "dark" : "light";
      root.dataset.theme = next;
      localStorage.setItem("dui-site-theme", next);
      syncThemeLabel();
    });
  }

  const i18n = {
    en: {
      releaseGuide: "Release documentation",
      supportHeading: "Analysis and rendering by platform",
      supportIntro: "dui reads types through the project’s official compiler. Rules recognize individual API calls. Each platform supplies its own evidence of what appeared on screen.",
      supportSurface: "Surface",
      supportInput: "What dui reads",
      supportResult: "What you can see",
      supportWeb: "Web",
      supportWebInput: "A local URL or static build. A TypeScript scan can also extract typed state from recognized calls.",
      supportWebResult: "Reachable pages captured in Chromium at the requested widths.",
      supportDesktop: "Desktop",
      supportDesktopInput: "Declared GPUI states or Electron web renderer content with optional IPC mocks.",
      supportDesktopResult: "GPUI state captures and element trees, or Electron renderer pages in Chromium.",
      supportMobile: "Mobile",
      supportMobileInput: "React Navigation, Expo Router and Flutter route declarations. Native captures need an Android device or emulator, or an iOS Simulator.",
      supportMobileResult: "Source cards with file locations, optionally linked to recordings. Selected SwiftUI and Compose components also export as interactive HTML previews.",
      supportTerminal: "Terminal (TUI)",
      supportTerminalInput: "Output from a command running in a pseudoterminal.",
      supportTerminalResult: "A terminal cell grid and its PNG at the configured sizes.",
      supportEvidence: "Analysis and rendering checks",
      openOriginal: "Open original",
      t3ThreadAlt: "T3 Code thread with a user request and the assistant’s test results",
      orcaHomeAlt: "Orca connection steps and the Pair desktop button",
      compassBookingAlt: "Compass booking details for an Alaska trip",
      iosExamples: "iOS screen examples",
      iosExamplesSub: "Captured on iPhone simulators. Select a screen to open the full-resolution image.",
      t3Thread: "Thread conversation",
      orcaHome: "Connect your desktop",
      compassBooking: "Alaska itinerary",
      captureRun: "View capture run",
      captureSources: "Sources and licenses",
      navOverview: "Overview",
      navExamples: "Examples",
      navArch: "Architecture",
      navCli: "CLI & MCP",
      eyebrow: "Rust 2024 · Fact-only UI verification",
      heroTitle: "Make the code the single source of truth for development and UI.",
      heroLead:
        "Today planning documents, Figma designs, and code drift apart. dui generates the screen map from the code, and UI edits go back into the same code.",
      btnDemo: "Open Interactive Canvas Demo",
      btnQuickstart: "Quickstart guide",
      btnExamples: "Explore examples",
      btnGithub: "GitHub",
      copy: "Copy",
      copied: "Copied",
      ossTitle: "Browse 6 trending open-source apps live in dui",
      ossSub:
        "Every project below is a real MIT-licensed open-source app captured with dui map across mobile (375px) and desktop (1280px). Click any card to inspect its 2D canvas, navigation edges, modals, and box model.",
      btnTrySourcePreview: "SwiftUI & Compose source map →",
      btnTryDiff: "Try 2D Inline Diff →",
      btnOpenAllCanvas: "Open Canvas Demo →",
      workflowTitle: "Zero-config workflow in three commands",
      workflowSub:
        "Works out of the box on Vite, Next.js App Router, and plain static HTML apps without writing a config file.",
      step1Title: "Auto-discover routes & modals",
      step2Title: "Review on Web or GPUI Desktop",
      step3Title: "Diff branches & drive from AI",
      factTitle: "Fact-only core. Zero guessed evidence.",
      factSub:
        "dui never invents screenshots or forces hardcoded aesthetic opinions onto diverse design systems.",
      factStatesTitle: "Honest verification states",
      factConformanceTitle: "Pluggable design conformance (v0.3.0)",
      tabOverview: "2D Screen Map",
      tabOverviewAlt:
        "dui 2D Web Canvas showing 11 full-page dashboard screens and a Preferences modal child node with obstacle-free gutter edges",
      tabOverviewCaption:
        "2D Web Canvas (dui open): Full-page true-aspect screens, auto-discovered modal/drawer child nodes, quick search, minimap, and obstacle-free gutter routing.",
      tabInspector: "Modal + Inspector",
      tabInspectorAlt:
        "dui Element Inspector displaying Element frames, Alignment guides, DevTools box model, computed oklch styles, and highlighted verified edge to the Theme Settings drawer",
      tabInspectorCaption:
        "Modal & Element Inspector: Auto-captured Theme Settings drawer with Element frames, Alignment guides, DevTools box model, and computed oklch styles.",
      tabDiff: "2D Inline Diff",
      tabDiffAlt:
        "dui 2D Inline Diff highlighting element-level changes on the map alongside Before, After, and Pixel difference overlays in the inspector",
      tabDiffCaption:
        "2D Inline Diff: Compare captures directly on the map with element-level change highlights, pixel-difference overlays, and screen verification evidence.",
      tabSourcePreview: "SwiftUI & Compose",
      tabSourcePreviewAlt:
        "dui map with two source cards showing native captures: Settings.swift on an iPhone 17 simulator and Settings.kt on an Android 15 emulator",
      tabSourcePreviewCaption:
        "Native SwiftUI & Compose: the same Settings component built with Apple SwiftUI and Jetpack Compose, captured on an iPhone simulator and an Android emulator, and attached to its source cards. Device recordings stay Unverified.",
      tabDesktopOverview: "macOS Desktop",
      tabDesktopOverviewAlt:
        "dui native GPUI macOS desktop viewer with transparent TitleBar, 11-screen full-page map, screen search, and gutter edges",
      tabDesktopOverviewCaption:
        "Native GPUI Desktop (dui desktop): macOS transparent TitleBar, full-bleed Metal canvas, screen search, cursor-anchored continuous zoom, and gutter edges.",
      tabDesktopInspector: "Desktop Inspector",
      tabDesktopInspectorAlt:
        "dui native GPUI desktop viewer showing selected dashboard screen with highlighted routes, screen verification, element tree, and box model",
      tabDesktopInspectorCaption:
        "Desktop Inspector: Clicking any card highlights connected routes and opens screen verification, the element tree, and box-model measurements.",
    },
    ko: {
      releaseGuide: "릴리스 문서",
      supportHeading: "플랫폼별 분석과 렌더링",
      supportIntro: "dui는 프로젝트의 공식 컴파일러에서 타입을 읽고, 규칙으로 개별 API 호출을 식별합니다. 화면에 나타난 결과는 각 플랫폼에서 관찰합니다.",
      supportSurface: "플랫폼",
      supportInput: "dui가 읽는 내용",
      supportResult: "확인할 수 있는 결과",
      supportWeb: "웹",
      supportWebInput: "로컬 URL이나 정적 빌드를 읽습니다. TypeScript 스캔은 규칙이 인식하는 호출에서 상태의 타입도 추출합니다.",
      supportWebResult: "Chromium에서 링크로 이동할 수 있는 페이지를 지정한 너비로 캡처합니다.",
      supportDesktop: "데스크톱",
      supportDesktopInput: "GPUI의 상태 선언 또는 Electron 웹 렌더러의 콘텐츠를 읽습니다. Electron에서는 IPC 목을 지정할 수 있습니다.",
      supportDesktopResult: "GPUI 상태별 캡처와 요소 트리, 또는 Chromium에서 캡처한 Electron 렌더러 페이지를 볼 수 있습니다.",
      supportMobile: "모바일",
      supportMobileInput: "React Navigation, Expo Router, Flutter의 경로 선언을 읽습니다. 네이티브 캡처에는 Android 기기·에뮬레이터 또는 iOS 시뮬레이터가 필요합니다.",
      supportMobileResult: "화면 카드에서 소스 위치와 연결한 캡처를 볼 수 있습니다. 일부 SwiftUI·Compose 컴포넌트는 조작 가능한 HTML 미리보기로도 내보낼 수 있습니다.",
      supportTerminal: "터미널(TUI)",
      supportTerminalInput: "가상 터미널에서 실행한 명령의 출력을 읽습니다.",
      supportTerminalResult: "설정한 크기의 터미널 문자 격자와 PNG를 볼 수 있습니다.",
      supportEvidence: "분석·렌더링 검증 근거",
      openOriginal: "원본 보기",
      t3ThreadAlt: "사용자 요청과 테스트 결과를 보여 주는 T3 Code 대화",
      orcaHomeAlt: "연결 단계와 데스크톱 페어링 버튼이 보이는 Orca 홈",
      compassBookingAlt: "알래스카 여행의 예약 정보가 보이는 Compass 화면",
      iosExamples: "iOS 화면 예시",
      iosExamplesSub: "iPhone 시뮬레이터에서 캡처했습니다. 화면을 선택하면 원본 크기로 열립니다.",
      t3Thread: "스레드 대화",
      orcaHome: "데스크톱 연결",
      compassBooking: "알래스카 여행 일정",
      captureRun: "캡처 실행 기록",
      captureSources: "출처 및 라이선스",
      navOverview: "개요",
      navExamples: "예제",
      navArch: "아키텍처",
      navCli: "CLI · MCP",
      eyebrow: "Rust 2024 · 객관적 UI 검증 시스템",
      heroTitle: "개발과 UI의 원천을 코드 하나로 합친다.",
      heroLead:
        "지금은 기획 문서, 피그마 디자인, 코드로 원천이 나뉘어 있습니다. dui는 화면 지도를 코드에서 만들고, UI를 고치면 같은 코드로 돌려보냅니다.",
      btnDemo: "인터랙티브 캔버스 데모 열기",
      btnQuickstart: "빠른 시작",
      btnExamples: "예제 보기",
      btnGithub: "GitHub",
      copy: "복사",
      copied: "복사됨",
      ossTitle: "최신 오픈소스 6개 프로젝트를 dui 캔버스에서 그대로 둘러보기",
      ossSub:
        "아래 6개는 MIT 라이선스를 확인한 실제 오픈소스 앱을 dui map으로 모바일(375px)·데스크톱(1280px) 캡처한 결과입니다. 카드를 누르면 2D 캔버스·연결선·모달·박스 모델을 바로 탐색하고 워크스페이스에서 프로젝트를 전환할 수 있습니다.",
      btnTrySourcePreview: "SwiftUI·Compose 소스 지도 →",
      btnTryDiff: "2D 인라인 Diff 보기 →",
      btnOpenAllCanvas: "캔버스 데모 열기 →",
      workflowTitle: "세 줄 명령으로 끝나는 무설정 워크플로우",
      workflowSub:
        "설정 파일 없이도 Vite, Next.js App Router, 정적 HTML 앱을 바로 탐색하고 캡처합니다.",
      step1Title: "경로와 모달 자동 탐색",
      step2Title: "웹·GPUI 데스크톱에서 검토",
      step3Title: "브랜치 Diff와 AI MCP 연동",
      factTitle: "사실만 기록하는 코어. 추측 없는 검증.",
      factSub:
        "dui는 스크린샷을 꾸며내지 않으며, 특정 디자인 기준을 강제하지 않습니다.",
      factStatesTitle: "정직한 검증 상태",
      factConformanceTitle: "플러그인형 디자인 적합성 (v0.3.0)",
      tabOverview: "2D 화면 지도",
      tabOverviewAlt:
        "11개 전체 페이지 대시보드와 Preferences 모달 자식 노드, 거터 연결선이 보이는 dui 2D 웹 캔버스",
      tabOverviewCaption:
        "2D 웹 캔버스(dui open): 원본 비율 전체 페이지 캡처, 자동 발견된 모달·드로어 자식 노드, 빠른 검색, 미니맵, 겹침 없는 거터 연결선.",
      tabInspector: "모달 · 인스펙터",
      tabInspectorAlt:
        "Theme Settings 드로어와 연결된 검증 선, 요소 프레임·정렬 가이드, 박스 모델, 계산된 oklch 스타일을 보여 주는 요소 인스펙터",
      tabInspectorCaption:
        "모달·요소 인스펙터: 자동 캡처한 Theme Settings 드로어에서 요소 프레임, 정렬 가이드, DevTools 박스 모델, 계산된 oklch 스타일을 확인합니다.",
      tabDiff: "2D 인라인 Diff",
      tabDiffAlt:
        "지도 카드 위 요소 단위 변경 강조와 인스펙터의 이전·이후·픽셀 차이 오버레이를 함께 보여 주는 2D 인라인 Diff",
      tabDiffCaption:
        "2D 인라인 Diff: 지도 위에서 이전·이후 캡처를 바로 비교하고, 요소 단위 변경 표시·픽셀 차이 오버레이·화면 검증 근거를 함께 확인합니다.",
      tabSourcePreview: "SwiftUI · Compose",
      tabSourcePreviewAlt:
        "iPhone 17 시뮬레이터의 Settings.swift와 Android 15 에뮬레이터의 Settings.kt 실제 캡처를 보여 주는 두 소스 카드의 dui 지도",
      tabSourcePreviewCaption:
        "네이티브 SwiftUI·Compose: 같은 Settings 컴포넌트를 Apple SwiftUI와 Jetpack Compose로 빌드해 iPhone 시뮬레이터와 Android 에뮬레이터에서 캡처하고 소스 카드에 연결했습니다. 기기 녹화는 미검증으로 표시됩니다.",
      tabDesktopOverview: "macOS 데스크톱",
      tabDesktopOverviewAlt:
        "투명 TitleBar, 11개 전체 페이지 지도, 화면 검색, 거터 연결선을 갖춘 네이티브 GPUI macOS 데스크톱 뷰어",
      tabDesktopOverviewCaption:
        "네이티브 GPUI 데스크톱(dui desktop): macOS 투명 TitleBar, 풀블리드 Metal 캔버스, 화면 검색, 커서 기준 연속 줌, 겹침 없는 거터 연결선.",
      tabDesktopInspector: "데스크톱 인스펙터",
      tabDesktopInspectorAlt:
        "선택한 대시보드 화면과 강조된 연결 경로, 화면 검증, 요소 트리, 박스 모델을 보여 주는 GPUI 데스크톱 인스펙터",
      tabDesktopInspectorCaption:
        "데스크톱 인스펙터: 카드를 선택하면 연결된 경로를 강조하고 화면 검증 결과, 요소 트리, 박스 모델 치수를 캔버스 좌표 이동 없이 띄웁니다.",
    },
    "zh-CN": {
      releaseGuide: "发布版文档",
      supportHeading: "各平台的分析与渲染",
      supportIntro: "dui 通过项目的官方编译器读取类型，通过规则识别具体的 API 调用。界面上实际显示的内容由各平台提供证据。",
      supportSurface: "平台",
      supportInput: "dui 读取的内容",
      supportResult: "可查看的结果",
      supportWeb: "Web",
      supportWebInput: "本地 URL 或静态构建产物。TypeScript 扫描还可从规则识别的调用中提取状态类型。",
      supportWebResult: "Chromium 按指定宽度捕获沿链接可以访问的页面。",
      supportDesktop: "桌面",
      supportDesktopInput: "已声明的 GPUI 状态或 Electron Web 渲染进程的内容。Electron 可配置 IPC 模拟。",
      supportDesktopResult: "可查看 GPUI 的状态截图和元素树，或在 Chromium 中捕获的 Electron 渲染页面。",
      supportMobile: "移动",
      supportMobileInput: "读取 React Navigation、Expo Router 和 Flutter 的路由声明。原生捕获需要 Android 设备或模拟器，或 iOS 模拟器。",
      supportMobileResult: "源码卡片显示文件位置，可关联录制的截图。部分 SwiftUI 和 Compose 组件也可导出为可交互的 HTML 预览。",
      supportTerminal: "终端（TUI）",
      supportTerminalInput: "在伪终端中运行命令所得的输出。",
      supportTerminalResult: "按配置的尺寸生成终端字符网格和 PNG。",
      supportEvidence: "分析与渲染的验证依据",
      openOriginal: "查看原图",
      t3ThreadAlt: "T3 Code 会话中的用户请求和测试结果",
      orcaHomeAlt: "Orca 的连接步骤和桌面配对按钮",
      compassBookingAlt: "Compass 中的阿拉斯加行程预订详情",
      iosExamples: "iOS 界面示例",
      iosExamplesSub: "这些画面来自 iPhone 模拟器。选择一个界面即可查看原始尺寸的图像。",
      t3Thread: "会话消息",
      orcaHome: "连接桌面端",
      compassBooking: "阿拉斯加行程",
      captureRun: "查看采集记录",
      captureSources: "来源与许可证",
      navOverview: "概览",
      navExamples: "示例",
      navArch: "架构",
      navCli: "CLI 与 MCP",
      eyebrow: "Rust 2024 · 基于客观事实的 UI 验证",
      heroTitle: "让代码成为开发与 UI 的单一事实来源。",
      heroLead:
        "今天，策划文档、Figma 设计和代码各自为政。dui 从代码生成界面地图，UI 修改再写回同一份代码。",
      btnDemo: "打开交互式画布演示",
      btnQuickstart: "快速上手",
      btnExamples: "查看示例",
      btnGithub: "GitHub",
      copy: "复制",
      copied: "已复制",
      ossTitle: "在 dui 画布中实时浏览 6 个热门开源项目",
      ossSub:
        "以下 6 个项目均已核验 MIT 许可证，由 dui map 在 375px 与 1280px 下真实捕获。点击任意卡片即可在 2D 画布中查看页面、导航连线、模态框与盒模型，并在工作区中切换项目。",
      btnTrySourcePreview: "SwiftUI 与 Compose 源码地图 →",
      btnTryDiff: "体验 2D 内联差异 →",
      btnOpenAllCanvas: "打开画布演示 →",
      workflowTitle: "三条命令完成零配置工作流",
      workflowSub:
        "无需编写配置文件，即可直接支持 Vite、Next.js App Router 和纯静态 HTML 应用。",
      step1Title: "自动发现路由与弹窗",
      step2Title: "在 Web 或 GPUI 桌面端评审",
      step3Title: "分支对比与 AI MCP 驱动",
      factTitle: "只记录事实的核心，不臆造证据。",
      factSub:
        "dui 不会虚构截图，也不向不同的设计系统强加硬编码审美规则。",
      factStatesTitle: "诚实的验证状态",
      factConformanceTitle: "可插拔设计规范校验 (v0.3.0)",
      tabOverview: "2D 界面地图",
      tabOverviewAlt:
        "展示 11 个整页仪表盘界面、Preferences 弹窗子节点和避让通道连线的 dui 2D Web 画布",
      tabOverviewCaption:
        "2D Web 画布（dui open）：按真实比例呈现整页截图，自动发现弹窗/抽屉子节点，支持快速搜索、缩略地图与避让卡片的通道连线。",
      tabInspector: "弹窗与检查器",
      tabInspectorAlt:
        "展示 Theme Settings 抽屉、已验证连线、元素边框、对齐参考线、盒模型与 oklch 计算样式的 dui 元素检查器",
      tabInspectorCaption:
        "弹窗与元素检查器：在自动捕获的 Theme Settings 抽屉中查看元素边框、对齐参考线、DevTools 盒模型与计算后的 oklch 样式。",
      tabDiff: "2D 内联差异",
      tabDiffAlt:
        "在界面地图卡片上高亮元素级变更，并在检查器中并排展示变更前、变更后与像素差异叠加层的 dui 2D 内联差异视图",
      tabDiffCaption:
        "2D 内联差异：直接在界面地图上对比两次捕获，高亮元素级变更，并查看像素差异叠加层与页面验证依据。",
      tabSourcePreview: "SwiftUI 与 Compose",
      tabSourcePreviewAlt:
        "dui 地图中的两张源码卡片，显示 iPhone 17 模拟器上的 Settings.swift 与 Android 15 模拟器上的 Settings.kt 真实截图",
      tabSourcePreviewCaption:
        "原生 SwiftUI 与 Compose：同一个 Settings 组件分别用 Apple SwiftUI 和 Jetpack Compose 构建，在 iPhone 模拟器与 Android 模拟器上截图并关联到源码卡片。设备录制保持未验证。",
      tabDesktopOverview: "macOS 桌面端",
      tabDesktopOverviewAlt:
        "带有透明 TitleBar、11 个整页界面地图、界面搜索与避让通道连线的原生 GPUI macOS 桌面查看器",
      tabDesktopOverviewCaption:
        "原生 GPUI 桌面端（dui desktop）：macOS 透明 TitleBar、全出血 Metal 画布、界面搜索、以光标为锚点的连续缩放与通道连线。",
      tabDesktopInspector: "桌面端检查器",
      tabDesktopInspectorAlt:
        "选中仪表盘界面后高亮关联路径并打开页面验证、元素树与盒模型测量的 GPUI 桌面检查器",
      tabDesktopInspectorCaption:
        "桌面端检查器：点击任意卡片即可高亮关联路径，并在不移动画布坐标的情况下打开页面验证、元素树与盒模型测量。",
    },
  };

  const tabs = Array.from(document.querySelectorAll("[data-showcase-tab]"));
  const img = document.querySelector("[data-showcase-img]");
  const caption = document.querySelector("[data-showcase-caption]");
  const syncShowcase = (tab) => {
    if (!tab) return;
    const dict = i18n[root.lang] || i18n.en;
    const altKey = tab.getAttribute("data-alt-key");
    const captionKey = tab.getAttribute("data-caption-key");
    const alt = (altKey && dict[altKey]) || tab.getAttribute("data-alt") || "";
    const text = (captionKey && dict[captionKey]) || tab.getAttribute("data-caption") || "";
    if (img) {
      img.setAttribute("src", tab.getAttribute("data-src") || "");
      img.setAttribute("alt", alt);
    }
    if (caption) caption.textContent = text;
  };
  for (const tab of tabs) {
    tab.addEventListener("click", () => {
      for (const other of tabs) other.setAttribute("aria-selected", String(other === tab));
      syncShowcase(tab);
    });
  }

  const langSelect = document.querySelector("[data-lang-select]");
  const applyLang = (lang) => {
    const dict = i18n[lang] || i18n.en;
    root.lang = lang;
    for (const el of document.querySelectorAll("[data-i18n]")) {
      const key = el.getAttribute("data-i18n");
      if (key && dict[key]) el.textContent = dict[key];
    }
    for (const image of document.querySelectorAll("img[data-i18n-alt]")) {
      const alt = dict[image.dataset.i18nAlt];
      if (alt) image.alt = alt;
    }
    for (const guide of document.querySelectorAll("[data-quickstart-guide], [data-install-guide]")) {
      const filename = lang === "en" ? "README.md" : `README.${lang}.md`;
      guide.href = new URL(filename + guide.hash, guide.href).href;
    }
    const activeTab = tabs.find((tab) => tab.getAttribute("aria-selected") === "true") || tabs[0];
    syncShowcase(activeTab);
    if (langSelect && langSelect.value !== lang) langSelect.value = lang;
  };
  const params = new URLSearchParams(location.search);
  const initialLang = params.get("lang") || localStorage.getItem("dui-site-lang") || "en";
  applyLang(i18n[initialLang] ? initialLang : "en");
  if (langSelect) {
    langSelect.addEventListener("change", () => {
      const next = langSelect.value;
      localStorage.setItem("dui-site-lang", next);
      applyLang(next);
    });
  }

  for (const btn of document.querySelectorAll("[data-copy]")) {
    btn.addEventListener("click", async () => {
      const text = btn.getAttribute("data-copy") || "";
      try {
        await navigator.clipboard.writeText(text);
        const dict = i18n[root.lang] || i18n.en;
        btn.textContent = dict.copied;
        setTimeout(() => {
          btn.textContent = dict.copy;
        }, 1400);
      } catch {
        // Ignore clipboard errors in headless environments.
      }
    });
  }

  for (const opener of document.querySelectorAll("[data-open-dialog]")) {
    opener.addEventListener("click", () => {
      const id = opener.getAttribute("data-open-dialog");
      const dlg = id ? document.getElementById(id) : null;
      if (dlg instanceof HTMLDialogElement && !dlg.open) dlg.showModal();
    });
  }
  for (const closer of document.querySelectorAll("[data-close-dialog]")) {
    closer.addEventListener("click", () => {
      const dlg = closer.closest("dialog");
      if (dlg instanceof HTMLDialogElement && dlg.open) dlg.close();
    });
  }

  for (const demoLink of document.querySelectorAll("[data-demo-link]")) {
    demoLink.addEventListener("click", (event) => {
      event.preventDefault();
      location.href = "./demo/";
    });
  }
  for (const runLink of document.querySelectorAll("[data-demo-run]")) {
    const key = runLink.getAttribute("data-demo-run");
    const compare = runLink.getAttribute("data-demo-compare");
    if (key) {
      runLink.addEventListener("click", (event) => {
        event.preventDefault();
        const qs = new URLSearchParams({ run: key });
        if (compare) qs.set("compare", compare);
        location.href = `./demo/?${qs.toString()}`;
      });
    }
  }
})();
