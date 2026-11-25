"""Exercise the pinned driver's install and retry order without building a simulator app."""

from pathlib import Path
import runpy
import subprocess
import sys


def main(source):
    prepare = runpy.run_path(str(Path(__file__).with_name("check-ios-t3-showcase.py")))["prepare_ios_launches"]
    driver = prepare((source / "scripts/mobile-showcase.ts").read_text(), '"check-ios-t3-showcase.py"')
    capture_ios = driver[driver.index("async function captureIos("):driver.index("function androidSdkTool(")]
    start = driver.index("      const pairingHost =")
    dispatch = driver[start:driver.index("      await validateCaptureSet(", start)]
    # Execute the actual capture and its caller. Replace only their host dependencies.
    setup = """
import assert from 'node:assert/strict';
import NodePath from 'node:path';
const noop = async () => {};
const capture = {device: {platform: 'ios'}, scenes: ['thread', 'review']};
const iosAppPath = 'Test.app', outputDirectory = '', showcaseConfig = {}, metroHost = '127.0.0.1';
const iosCleanups = [], ANDROID_PACKAGE = 'fixture', DEV_CLIENT_LAUNCH_FLAGS = '', IOS_READY_FILENAME = 'ready';
const showcaseEnvironments = [{port: 101, baseDir: 'one'}, {port: 102, baseDir: 'two'}, {port: 103, baseDir: 'three'}];
const ensureIosSimulator = async () => ({simulator: {udid: 'simulator', state: 'Shutdown'}});
const suppressIosSystemFollowUps = noop, normalizeIosSimulator = noop, delay = noop, finalizeCapture = noop;
const iosAppContainer = () => '', showcaseCaptureDirectory = () => '', NodeFSP = {rm: noop, writeFile: noop};
let installed = false, issued = 0, waits = 0;
const launches = [];
const issuePairingCredential = async base => {
    assert.ok(installed, 'fixture links were issued before installation');
    return base + '-' + (++issued);
};
const buildShowcasePairingUrl = (host, port, grant) => ({host, port, grant});
async function runCommand(command, args) {
    if (command !== 'xcrun') return;
    if (args[1] === 'install') installed = true;
    if (args[1] === 'launch') launches.push(JSON.parse(args[args.indexOf('--showcasePairingUrl') + 1]));
}
async function waitForIosShowcaseScene() {
    if (waits++ === 0) throw Error('simulate first launch timing out');
}
"""
    assertions = """
assert.equal(launches.length, 2);
for (const links of launches) {
    assert.deepEqual(links.map(link => link.port), showcaseEnvironments.map(env => env.port));
    assert.ok(links.every(link => link.host === '127.0.0.1'));
}
assert.equal(new Set(launches.flat().map(link => link.grant)).size, launches.flat().length);
console.log('Initial launch and retry each used fresh grants for every fixture server');
"""
    subprocess.run(["node", "--input-type=module-typescript"], check=True, text=True,
                   input="\n".join((setup, capture_ios, dispatch, assertions)))


if __name__ == "__main__":
    main(Path(sys.argv[1]))
