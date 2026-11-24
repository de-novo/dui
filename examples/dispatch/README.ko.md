# Dispatch 픽업 보드

[English](README.md) · [한국어](README.ko.md) · [简体中文](README.zh-CN.md)

픽업 보드에서 수거 상세로 이동한 뒤 기사 인계 안내를 엽니다. HTML, CSS와 네이티브 대화상자로 구성한 예제입니다. [녹화 영상 보기 (MP4, 20초)](../recordings/dispatch.mp4)

## 캡처와 요소 검사

dui, Node.js, Chromium을 설치하고 저장소 루트에서 시작합니다. 같은 프로젝트 이력에 두 번의 캡처를 남기기 위해 임시 복사본을 사용합니다.

```sh
work=$(mktemp -d)
cp -R examples/dispatch "$work/dispatch"
cd "$work/dispatch"
dui map . > "$work/before.json"
before=$(node -p 'require(process.argv[1]).output' "$work/before.json")
dui open "$before"
```

지도에서 대화상자 카드를 열어 인계 안내가 캡처됐는지 확인합니다. 픽업 보드 카드를 선택하고 **Element Inspector → Rendered elements**에서 제목을 고르면 실제 측정된 박스가 표시됩니다.

## 변경 비교

Ctrl+C로 뷰어를 종료합니다. 같은 셸에서 제목을 바꾸고 다시 캡처합니다.

```sh
node -e 'const fs = require("node:fs"); const p = "index.html"; fs.writeFileSync(p, fs.readFileSync(p, "utf8").replace("<h1>Pickup board</h1>", "<h1>Pickup board · revised</h1>"))'
dui map . > "$work/after.json"
after=$(node -p 'require(process.argv[1]).output' "$work/after.json")
dui diff "$before" "$after"
dui open "$before" "$after"
```

**Map options → Compare captures**에서 변경 전후의 실행을 고릅니다. CLI에서는 캡처한 너비별로 제목이 어떻게 바뀌었는지 확인할 수 있습니다.

## 데스크톱에서 열기

웹 뷰어를 종료하고 GPUI 데스크톱 빌드로 첫 캡처를 엽니다. 수거 상세 카드를 선택하면 연결된 화면을 확인할 수 있습니다.

```sh
dui desktop --run "$(node -p 'require(process.argv[1]).snapshot.id' "$work/before.json")"
```

## 자동 검증

Rust, Node.js 26, Chromium이 설치된 환경에서 저장소 루트에서 실행합니다.

```sh
npm ci
npm ci --prefix apps/canvas
npm run check:example
```

CI는 캡처한 너비마다 Chromium으로 링크를 따라가고 대화상자를 연 뒤 아카이브, Inspector 선택 결과, 변경 전후 Diff를 검사합니다. 네이티브 렌더링까지 확인하려면 Cargo로 `apps/desktop/Cargo.toml`을 빌드한 뒤 `npm run check:example -- --desktop`을 실행합니다. 영상 마지막의 개요·상세 화면은 macOS Metal로 렌더링한 스냅샷입니다.
