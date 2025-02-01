#!/usr/bin/env python3
import os
import subprocess

root_dir = os.path.dirname(__file__)

# A couple of steps are necessary to get this build working which makes it slightly
# nonstandard compared to most other builds.
#
# * First, the Rust standard library needs to be recompiled with atomics
#   enabled. to do that we use Cargo's unstable `-Zbuild-std` feature.
#
# * Next we need to compile everything with the `atomics` and `bulk-memory`
#   features enabled, ensuring that LLVM will generate atomic instructions,
#   shared memory, passive segments, etc.

# os.environ.update(
#     {"RUSTFLAGS": "-C target-feature=+bulk-memory,+mutable-globals"}
# )

print("now: rustc compilation")

subprocess.run(
    [
        "cargo",
        "build",
        "--release",
        "--target",
        "wasm32-unknown-unknown"
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
        os.path.join(root_dir, "pkg", "gba_rust_wasm_bg.wasm"),
        "-O4",
        "-o",
        os.path.join(root_dir, "pkg", "gba_rust_wasm_bg.wasm"),
    ],
    cwd=root_dir,
).check_returncode()
