#!/usr/bin/env python3
"""Prepare, or explicitly run, one render test without desktop/GPU access.

Preparation never enables the native-test gate or initializes a render driver.
An explicit --run executes once in bubblewrap, using only Mesa lavapipe.
This checks rendering semantics, not physical GPU/driver compatibility.
"""

import argparse
import datetime
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys


REPO = Path(__file__).resolve().parents[1]


def executable(name):
    value = shutil.which(name)
    if value is None:
        raise RuntimeError(f"required executable missing: {name}")
    return str(Path(value).resolve())


def sandbox(bwrap, output, icd, software=False):
    command = [
        bwrap, "--unshare-all", "--die-with-parent", "--new-session",
        "--cap-drop", "ALL", "--ro-bind", "/", "/", "--dev", "/dev",
        "--proc", "/proc", "--tmpfs", "/tmp", "--tmpfs", "/run",
        "--bind", str(output), str(output), "--chdir", str(REPO), "--clearenv",
    ]
    environment = {
        "VK_DRIVER_FILES": str(icd),
        "VK_ICD_FILENAMES": str(icd),
        "VK_LOADER_LAYERS_DISABLE": "~implicit~",
        "XIV_RENDER_TEST_SOFTWARE_ONLY": "1",
        "XIV_WEAPON_RENDER_SNAPSHOT_DIR": str(output),
        "XDG_CACHE_HOME": "/tmp/cache",
        "LP_NUM_THREADS": "4",
        "RAYON_NUM_THREADS": "4",
    }
    # /run is hidden; resolve loader search paths before entering the namespace.
    libraries = [str(Path(p).resolve()) for p in os.environ.get("LD_LIBRARY_PATH", "").split(":") if p]
    if libraries:
        environment["LD_LIBRARY_PATH"] = ":".join(libraries)
    if software:
        environment["XIV_ALLOW_GPU_TESTS"] = "1"
    for name, value in environment.items():
        command.extend(["--setenv", name, value])
    return command + ["--"]


def test_limits():
    resource.setrlimit(resource.RLIMIT_NOFILE, (4096, 4096))
    resource.setrlimit(resource.RLIMIT_AS, (8 * 1024**3, 8 * 1024**3))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("test", help="one exact ignored test name in vfx_render_semantics")
    parser.add_argument("--run", action="store_true", help="explicitly execute once; default only prepares")
    parser.add_argument("--icd", type=Path, default=Path("/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json"))
    parser.add_argument("--timeout", type=int, default=180)
    args = parser.parse_args()
    if not 1 <= args.timeout <= 600:
        parser.error("timeout must be 1..600 seconds")
    icd = args.icd.resolve(strict=True)
    manifest = json.loads(icd.read_text())
    library = Path(manifest["ICD"]["library_path"])
    if not library.is_absolute():
        library = icd.parent / library
    library = library.resolve(strict=True)
    if not icd.name.startswith("lvp_icd.") or library.name != "libvulkan_lvp.so":
        raise RuntimeError("only a Mesa lavapipe manifest/library is accepted")
    bwrap = executable("bwrap")
    stamp = datetime.datetime.now().astimezone().strftime("%Y%m%dT%H%M%S%z")
    output = REPO / "target" / "software-render-tests" / stamp
    output.mkdir(parents=True, exist_ok=False)
    base = sandbox(bwrap, output, icd)
    preflight_code = """
import os, json
assert not os.path.exists('/dev/dri')
assert not any(n.startswith('nvidia') for n in os.listdir('/dev'))
assert not os.listdir('/run')
assert not os.listdir('/tmp')
assert 'DISPLAY' not in os.environ and 'WAYLAND_DISPLAY' not in os.environ
assert 'XIV_ALLOW_GPU_TESTS' not in os.environ
assert os.environ['VK_DRIVER_FILES'] == os.environ['VK_ICD_FILENAMES']
assert int(next(l.split()[1] for l in open('/proc/self/status') if l.startswith('CapEff:')), 16) == 0
print(json.dumps({'devices': os.listdir('/dev'), 'run': os.listdir('/run'), 'gpu_gate': 'disabled', 'capabilities': 0}))
"""
    preflight = subprocess.check_output(base + [str(Path(sys.executable).resolve()), "-c", preflight_code], text=True, timeout=15)
    print("Isolation preflight:", preflight.strip(), flush=True)
    build = subprocess.run([
        executable("cargo"), "test", "--locked", "--features", "render-test-support",
        "--test", "vfx_render_semantics", "--no-run", "--message-format=json",
    ], cwd=REPO, stdout=subprocess.PIPE, text=True, check=True)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
    binaries = [a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("target", {}).get("name") == "vfx_render_semantics" and a.get("executable")]
    if len(binaries) != 1:
        raise RuntimeError("expected exactly one render-test binary")
    listing = subprocess.check_output(base + [binaries[0], "--list", "--ignored"], text=True, timeout=15)
    if args.test + ": test" not in listing.splitlines():
        raise RuntimeError(f"exact ignored test not found: {args.test}")
    command = sandbox(bwrap, output, icd, software=True) + [
        binaries[0], args.test, "--exact", "--ignored", "--test-threads=1", "--nocapture",
    ]
    report = {"test": args.test, "icd": str(icd), "library": str(library), "preflight": json.loads(preflight), "command": command, "timeout_seconds": args.timeout, "executed": False, "hardware_gpu_verified": False}
    report_path = output / "report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print("Prepared single-test command:", json.dumps(command), flush=True)
    print("Report:", report_path, flush=True)
    if not args.run:
        print("Prepared only; no rendering driver was initialized.")
        return 0
    report["executed"] = True
    report["started_at"] = datetime.datetime.now().astimezone().isoformat()
    with (output / "test.log").open("w") as log:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, close_fds=True, preexec_fn=test_limits)
        try:
            report["exit_code"] = process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            report["exit_code"] = 124
            report["timed_out"] = True
    report["finished_at"] = datetime.datetime.now().astimezone().isoformat()
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print((output / "test.log").read_text(), end="")
    return report["exit_code"]


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Software render preparation failed: {error}", file=sys.stderr)
        sys.exit(1)
