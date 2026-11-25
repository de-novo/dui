"""Build the real CLI across tag changes without modifying the original checkout."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib

root = Path(__file__).resolve().parents[1]
base = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
stable = f"v{base}"
major = int(base.split(".")[0]) + 1

with tempfile.TemporaryDirectory(prefix="dui-versions-") as temp:
    repo = Path(temp) / "source"
    subprocess.run(["git", "clone", "--quiet", "--no-tags", str(root), str(repo)], check=True)
    env = {**os.environ, "DUI_REPO_URL": str(repo), "CARGO_TARGET_DIR": str(Path(temp) / "target")}
    env.pop("DUI_VERSION", None)

    def run(*args):
        return subprocess.check_output(args, cwd=repo, env=env, text=True).strip()

    run("git", "switch", "-qc", "version-test")
    run("git", "config", "user.name", "Version fixture")
    run("git", "config", "user.email", "version@example.invalid")
    for modules in ["node_modules", "apps/canvas/node_modules"]:
        (repo / modules).symlink_to(root / modules, target_is_directory=True)

    def check_build(version, tag, latest, update):
        run("cargo", "build", "--locked", "--quiet", "--bin", "dui")
        cli = Path(env["CARGO_TARGET_DIR"]) / "debug/dui"
        actual = json.loads(run(cli, "--version"))
        assert actual == {"name": "dui", "version": version, "tag": tag}, actual
        initialize = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
        reply = subprocess.check_output([cli, "mcp"], cwd=repo, env=env, text=True,
                                        input=json.dumps(initialize) + "\n")
        assert json.loads(reply)["result"]["serverInfo"]["version"] == version
        check = json.loads(run(cli, "update", "--check"))
        assert check["current"] == f"v{version}" and check["latest"] == latest, check
        assert check["updateAvailable"] == update, check
        assert check["nextTag"] == (latest if update else None), check

    def development():
        return base + "-dev+" + run("git", "rev-parse", "--short=12", "HEAD")

    def check_pages(expected, directory):
        output = Path(temp) / directory
        run("node", "scripts/stage-pages.mts", str(output))
        assert run("sh", str(output / "install"), "--print-ref") == expected
        assert f'/blob/{expected}/README.md"' in (output / "index.html").read_text()
        for page in output.glob("*.html"):
            assert f"data-install-ref>{expected}</span>" in page.read_text(), page

    run("git", "tag", stable + "-rc.1")
    check_build(development(), None, None, False)
    check_pages("main", "empty")
    run("git", "tag", "-a", stable, "-m", "local release")
    check_build(base, stable, stable, False)
    run("git", "pack-refs", "--all")
    check_build(base, stable, stable, False)
    run("git", "commit", "--allow-empty", "-qm", "first development build")
    check_build(development(), None, stable, True)
    run("git", "commit", "--allow-empty", "-qm", "next development build")
    check_build(development(), None, stable, True)
    for tag in [f"v{major}.0.2", f"v{major}.0.10", f"v0{major}.0.99"]:
        run("git", "tag", tag, "HEAD~1")
    check_build(development(), None, f"v{major}.0.10", True)
    check_pages(f"v{major}.0.10", "released")
    run("git", "tag", f"v{major}.0.0")
    mismatch = subprocess.run(["cargo", "build", "--locked", "--bin", "dui"], cwd=repo,
                              env=env, capture_output=True, text=True)
    assert mismatch.returncode and "release tag must match" in mismatch.stderr, mismatch.stderr
    print("Verified development revisions, exact releases, updates, tag mismatch, installer and Pages")
