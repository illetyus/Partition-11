# EXT image parser fuzz target

The ext_image target drives the pinned read-only partition11-ext-reader
through open, bounded root-directory listing, one nested-directory lookup, and
small regular-file reads. Each fuzz input is written to a process-specific
temporary file and removed after inspection; the corpus and physical disks are
never used as output targets.

CI copies the clean and recovery-pending e2fsprogs 1.47.3 fixtures into a
temporary corpus, builds with the pinned cargo-fuzz 0.13.2 tool, and runs a
60-second bounded libFuzzer smoke with a 16 MiB maximum input. Crash artifacts
are uploaded by the workflow when the fuzz job fails.

For a local run, install nightly-2026-10-17 and cargo-fuzz 0.13.2, open an
x64 Visual Studio Developer PowerShell with the MSVC AddressSanitizer
component enabled, and run:

    New-Item -ItemType Directory -Force fuzz/corpus/ext_image | Out-Null
    Copy-Item crates/ext-reader/tests/fixtures/ext4-e2fsprogs-1.47.3.img fuzz/corpus/ext_image/e2fsprogs-1.47.3.img
    Copy-Item crates/ext-reader/tests/fixtures/ext4-e2fsprogs-1.47.3-not-clean.img fuzz/corpus/ext_image/e2fsprogs-1.47.3-not-clean.img
    cargo +nightly-2026-10-17 fuzz build ext_image
    cargo +nightly-2026-10-17 fuzz run ext_image -- -max_total_time=60 -timeout=10 -max_len=16777216

The CI smoke is a bounded regression gate, not a substitute for longer
coverage-guided fuzz campaigns or hardware acceptance.
