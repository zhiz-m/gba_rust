#!/usr/bin/env python3

# adapted from https://github.com/rustwasm/wasm-bindgen/blob/main/examples/raytrace-parallel/build.py

import os
import subprocess

root_dir = os.path.dirname(__file__)

os.environ.update(
    {"RUSTFLAGS": "-C target-feature=+simd128"}
)

print("now: rustc compilation")

subprocess.run(
    [
        "cargo",
        "build",
        "--release",
        "--target",
        "wasm32-unknown-unknown",
        "-Zbuild-std=std,panic_abort",
    ],
    cwd=root_dir,
).check_returncode()

print("now: wasm-bindgen pass")

# Note the usage of `--target no-modules` here which is required for passing
# the memory import to each Wasm module.
subprocess.run(
    [
        "wasm-bindgen",
        os.path.join(
            root_dir,
            "..",
            "..",
            "target",
            "wasm32-unknown-unknown",
            "release",
            "gba_rust_wasm.wasm",
        ),
        "--out-dir",
        os.path.join(root_dir, "pkg"),
        "--target",
        "no-modules",
    ],
    cwd=root_dir,
).check_returncode()

print("now: wasm-opt pass")

subprocess.run(
    [
        "wasm-opt",
        "--enable-bulk-memory",
        "--enable-reference-types",
        os.path.join(root_dir, "pkg", "gba_rust_wasm_bg.wasm"),
        "--debuginfo",
        "-O4",
        "-o",
        os.path.join(root_dir, "pkg", "gba_rust_wasm_bg.wasm"),
    ],
    cwd=root_dir,
).check_returncode()
