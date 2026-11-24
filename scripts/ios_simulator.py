"""Run a command on a fresh simulator. The command reads DUI_IOS_UDID, DUI_IOS_DEVICE and DUI_IOS_RUNTIME."""
from contextlib import contextmanager
from dataclasses import dataclass
import json
import os
import subprocess
import sys
import uuid


@dataclass(frozen=True)
class IPhone:
    udid: str
    name: str
    runtime: str


@contextmanager
def fresh_iphone(label):
    simctl = ["xcrun", "simctl"]
    inventory = subprocess.check_output([*simctl, "list", "devices", "available", "-j"], text=True)
    candidates = [
        (runtime, device) for runtime, group in json.loads(inventory)["devices"].items()
        for device in group if ".iOS-" in runtime and "iPhone" in device.get("deviceTypeIdentifier", "")
    ]
    if not candidates:
        raise RuntimeError("A bootable iOS runtime and an iPhone device type are required")
    runtime, device = max(candidates, key=lambda pair: (
        tuple(map(int, pair[0].split(".iOS-")[1].split("-"))), pair[1]["name"]
    ))
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
