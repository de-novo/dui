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
    },
    ko: {
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
    },
    "zh-CN": {
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
    },
  };

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

  const tabs = Array.from(document.querySelectorAll("[data-showcase-tab]"));
  const img = document.querySelector("[data-showcase-img]");
  const caption = document.querySelector("[data-showcase-caption]");
  for (const tab of tabs) {
    tab.addEventListener("click", () => {
      for (const other of tabs) other.setAttribute("aria-selected", String(other === tab));
      if (img) {
        img.setAttribute("src", tab.getAttribute("data-src") || "");
        img.setAttribute("alt", tab.getAttribute("data-alt") || "");
      }
      if (caption) caption.textContent = tab.getAttribute("data-caption") || "";
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
