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
      eyebrow: "v0.2.0 · Rust 2024 · Fact-only UI verification",
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
      eyebrow: "v0.2.0 · Rust 2024 · 객관적 UI 검증 시스템",
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
      eyebrow: "v0.2.0 · Rust 2024 · 基于客观事实的 UI 验证",
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
