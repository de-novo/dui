# Install the dui binary

## Shell installer

Once the repository and release assets are public, install the latest release directly from GitHub.

```sh
curl -fsSL https://github.com/de-novo/dui/releases/latest/download/install.sh | bash
```

The installer downloads the archive and checksum from its own release tag. The default destination is `$HOME/.local/bin/dui`. To change the destination, set `DUI_INSTALL_DIR` on the installer side of the pipe, for example `| DUI_INSTALL_DIR="$PWD/bin" bash`.

For offline installation, place `install.sh`, the archive, and its checksum in one directory and run `sh install.sh` there.

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
