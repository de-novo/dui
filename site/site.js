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
      navOverview: "Overview",
      navExamples: "Examples",
      navArch: "Architecture",
      navCli: "CLI & MCP",
      eyebrow: "v0.2.0 · Rust 2024 · Fact-only UI verification",
      heroTitle: "Render every screen, state, modal, and width from real code.",
      heroLead:
        "dui reads your app through its official compiler, captures every screen × state × modal × width at true aspect ratio, and gives humans a 2D canvas while AI agents consume structured JSON over CLI and MCP.",
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
      navOverview: "개요",
      navExamples: "예제",
      navArch: "아키텍처",
      navCli: "CLI · MCP",
      eyebrow: "v0.2.0 · Rust 2024 · 객관적 UI 검증 시스템",
      heroTitle: "실제 코드의 모든 화면 · 상태 · 모달 · 폭을 한눈에.",
      heroLead:
        "dui는 공식 컴파일러와 웹 표준 탐색으로 앱의 모든 화면 × 상태 × 모달 × 폭을 실제 비율로 캡처합니다. 사람은 2D 캔버스에서 리뷰하고, AI 에이전트는 CLI와 MCP로 정형 JSON을 읽습니다.",
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
      navOverview: "概览",
      navExamples: "示例",
      navArch: "架构",
      navCli: "CLI 与 MCP",
      eyebrow: "v0.2.0 · Rust 2024 · 基于客观事实的 UI 验证",
      heroTitle: "从真实代码渲染每个页面、状态、模态框与宽度。",
      heroLead:
        "dui 通过官方编译器与标准导航发现捕获应用的每个页面 × 状态 × 模态框 × 宽度。人类在 2D 画布上直观审阅，AI 代理通过 CLI 与 MCP 读取结构化 JSON。",
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
    demoLink.setAttribute("href", "./demo/");
  }
  for (const runLink of document.querySelectorAll("[data-demo-run]")) {
    const key = runLink.getAttribute("data-demo-run");
    if (key) runLink.setAttribute("href", `./demo/?run=${encodeURIComponent(key)}`);
  }
})();
