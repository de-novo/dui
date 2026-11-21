//! Trilingual UI strings (`en`, `ko`, `zh-CN`) for the GPUI desktop viewer.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiLang {
    En,
    Ko,
    ZhCn,
}

impl UiLang {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ko" | "ko-kr" => Self::Ko,
            "zh" | "zh-cn" | "zh-hans" => Self::ZhCn,
            _ => Self::En,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::En => Self::Ko,
            Self::Ko => Self::ZhCn,
            Self::ZhCn => Self::En,
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::Ko => "한국어",
            Self::ZhCn => "中文",
        }
    }
}

pub struct Strings {
    pub app_title: &'static str,
    pub refresh: &'static str,
    pub overview: &'static str,
    pub detail: &'static str,
    pub diff: &'static str,
    pub dark: &'static str,
    pub light: &'static str,
    pub fit: &'static str,
    pub workspace_empty: &'static str,
    pub inspector: &'static str,
    pub states_and_widths: &'static str,
    pub transitions: &'static str,
    pub no_transitions: &'static str,
    pub review_comments: &'static str,
    pub comment_placeholder: &'static str,
    pub post: &'static str,
    pub resolve: &'static str,
    pub reopen: &'static str,
    pub no_comments: &'static str,
    pub target_screen: &'static str,
    pub target_width: &'static str,
    pub target_transition: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub overlay: &'static str,
    pub need_two_runs: &'static str,
    pub no_changes: &'static str,
    pub open_detail: &'static str,
    pub back_to_map: &'static str,
}

pub fn strings(lang: UiLang) -> Strings {
    match lang {
        UiLang::En => Strings {
            app_title: "dui",
            refresh: "Refresh",
            overview: "Screen map",
            detail: "Detail",
            diff: "Compare",
            dark: "Dark",
            light: "Light",
            fit: "Fit",
            workspace_empty: "No captures in ~/.dui/dui.db yet.",
            inspector: "Inspector",
            states_and_widths: "States & widths",
            transitions: "Links",
            no_transitions: "No outgoing transitions recorded.",
            review_comments: "Review comments",
            comment_placeholder: "Leave review feedback...",
            post: "Post",
            resolve: "Resolve",
            reopen: "Reopen",
            no_comments: "No review comments on this screen yet.",
            target_screen: "Screen",
            target_width: "Width",
            target_transition: "Transition",
            before: "Before",
            after: "After",
            overlay: "Pixel diff",
            need_two_runs: "Capture at least two runs for this project to compare changes.",
            no_changes: "All cells match between the selected captures.",
            open_detail: "Inspect 1:1",
            back_to_map: "Map",
        },
        UiLang::Ko => Strings {
            app_title: "dui",
            refresh: "새로고침",
            overview: "화면 지도",
            detail: "상세",
            diff: "비교",
            dark: "다크",
            light: "라이트",
            fit: "맞춤",
            workspace_empty: "~/.dui/dui.db에 저장된 캡처가 없습니다.",
            inspector: "인스펙터",
            states_and_widths: "상태 및 폭",
            transitions: "전환 링크",
            no_transitions: "기록된 전환 링크가 없습니다.",
            review_comments: "리뷰 코멘트",
            comment_placeholder: "리뷰 의견을 입력하세요...",
            post: "등록",
            resolve: "해결",
            reopen: "다시 열기",
            no_comments: "이 화면에 등록된 코멘트가 없습니다.",
            target_screen: "화면",
            target_width: "폭",
            target_transition: "전환",
            before: "이전",
            after: "이후",
            overlay: "픽셀 차이",
            need_two_runs: "변경 비교를 보려면 이 프로젝트의 캡처가 2회 이상 필요합니다.",
            no_changes: "두 캡처의 모든 칸이 동일합니다.",
            open_detail: "1:1 상세",
            back_to_map: "지도로",
        },
        UiLang::ZhCn => Strings {
            app_title: "dui",
            refresh: "刷新",
            overview: "界面地图",
            detail: "详情",
            diff: "对比",
            dark: "深色",
            light: "浅色",
            fit: "适应",
            workspace_empty: "~/.dui/dui.db 中暂无捕获记录。",
            inspector: "检查器",
            states_and_widths: "状态与宽度",
            transitions: "跳转链接",
            no_transitions: "暂无已记录的跳转。",
            review_comments: "评审评论",
            comment_placeholder: "输入评审意见...",
            post: "提交",
            resolve: "解决",
            reopen: "重新打开",
            no_comments: "此界面暂无评审评论。",
            target_screen: "界面",
            target_width: "宽度",
            target_transition: "跳转",
            before: "变更前",
            after: "变更后",
            overlay: "像素差异",
            need_two_runs: "此项目需要至少两次捕获记录才能对比变更。",
            no_changes: "两次捕获的所有单元格均一致。",
            open_detail: "1:1 详情",
            back_to_map: "返回地图",
        },
    }
}
