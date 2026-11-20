# Install the dui binary

## Shell installer

From a release directory containing `install.sh`, the archive for your target, and its checksum, run `sh install.sh`. The default destination is `$HOME/.local/bin/dui`. To choose another directory, run `DUI_INSTALL_DIR="$PWD/bin" sh install.sh`.

For a hosted release, set `DUI_RELEASE_URL` to its trusted HTTPS asset directory. All three files must belong to the same release.

```sh
curl --proto '=https' --proto-redir '=https' -fsSL "$DUI_RELEASE_URL/install.sh" | sh -s -- "$DUI_RELEASE_URL"
```

## Manual installation

Choose the archive for your operating system and CPU. For example, `aarch64-apple-darwin` is macOS on Apple silicon. Place the archive and its matching `.sha256` file in the same directory. Set `archive` to the downloaded filename.

```sh
archive=dui-0.1.0-aarch64-apple-darwin.tar.gz
shasum -a 256 -c "$archive.sha256"
tar -xzf "$archive"
mkdir -p "$HOME/.local/bin"
install -m 755 "${archive%.tar.gz}/dui" "$HOME/.local/bin/dui"
"$HOME/.local/bin/dui" --version
```

On Linux, use `sha256sum -c "$archive.sha256"` to verify the checksum. Add `$HOME/.local/bin` to your shell's `PATH` to invoke the binary as `dui`.

## Capture an application

Install Node.js 26 and the target app’s dependencies, including Vite 8 and the official TypeScript 6.x or 7.x compiler. Web capture also requires Chromium or Google Chrome. Set `DUI_CHROMIUM` to the executable path if needed.

Save the following as `dui.json` in the app directory. Replace the selector with an element that appears once your content has loaded.

```json
{ "path": "/", "widths": [375, 1280], "readySelector": "main article" }
```

```sh
dui run . dui.json .dui/capture
```

Open `.dui/capture/graph.html` to browse the screenshots. Read `.dui/capture/report.json` for per-cell results. For MCP clients, use the installed binary as the stdio command with `["mcp"]` as its arguments.
