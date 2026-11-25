"""Run a command on a fresh simulator. The command reads DUI_IOS_UDID, DUI_IOS_DEVICE and DUI_IOS_RUNTIME."""
from contextlib import contextmanager
from dataclasses import dataclass
import json
import os
import re
import subprocess
import sys
import uuid


@dataclass(frozen=True)
class IPhone:
    udid: str
    name: str
    runtime: str


def select_iphone(runtimes, available_devices):
    candidates = []
    for runtime in runtimes:
        identifier = runtime["identifier"]
        if not runtime["isAvailable"] or ".iOS-" not in identifier:
            continue
        version = re.match(r"\d+(?:\.\d+)*", runtime["version"])
        if version is None:
            continue
        release = tuple(map(int, version.group().split(".")))
        stable = not re.search(r"beta|preview|release candidate|\bRC\b",
                               f'{runtime["name"]} {runtime["version"]}', re.IGNORECASE)
        for device in available_devices.get(identifier, []):
            model = device.get("deviceTypeIdentifier", "").split(".")[-1]
            if not model.startswith("iPhone-"):
                continue
            base_model = re.fullmatch(r"iPhone-(\d+)", model)
            generation = int(base_model[1]) if base_model else 0
            priority = (stable, release, bool(base_model), generation, model, identifier)
            candidates.append((priority, identifier, device))
    if not candidates:
        raise RuntimeError("A bootable iOS runtime and an iPhone device type are required")
    _, identifier, device = max(candidates, key=lambda candidate: candidate[0])
    return identifier, device


@contextmanager
def fresh_iphone(label):
    simctl = ["xcrun", "simctl"]
    runtimes = subprocess.check_output([*simctl, "list", "runtimes", "-j"], text=True)
    devices = subprocess.check_output([*simctl, "list", "devices", "available", "-j"], text=True)
    runtime, device = select_iphone(json.loads(runtimes)["runtimes"], json.loads(devices)["devices"])
    device_name = f"dui-{label}-{uuid.uuid4().hex[:8]}"
    udid = subprocess.check_output(
        [*simctl, "create", device_name, device["deviceTypeIdentifier"], runtime], text=True
    ).strip()
    try:
        subprocess.run([*simctl, "bootstatus", udid, "-b"], check=True)
        yield IPhone(udid, device["name"], runtime)
    finally:
        subprocess.run([*simctl, "shutdown", udid], check=False)
        subprocess.run([*simctl, "delete", udid], check=True)


if __name__ == "__main__":
    if len(sys.argv) < 4 or sys.argv[2] != "--":
        sys.exit("Usage: ios_simulator.py LABEL -- COMMAND [ARG...]")
    with fresh_iphone(sys.argv[1]) as device:
        status = subprocess.run(
            sys.argv[3:],
            env={**os.environ, "DUI_IOS_UDID": device.udid, "DUI_IOS_DEVICE": device.name,
                 "DUI_IOS_RUNTIME": device.runtime},
        ).returncode
    sys.exit(status)
