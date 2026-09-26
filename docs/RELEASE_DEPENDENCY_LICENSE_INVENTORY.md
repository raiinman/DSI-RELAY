# Rust release dependency and license inventory

Status: factual inventory for the current Windows `relay.exe` and `relayd.exe` dependency graph, 2026-09-26. It does not approve distribution or select RELAY's own license.

## Source and scope

- Source: `Cargo.lock` SHA-256 `8f46c4a561e2716ffb43d0a9ade13979d9ee021147a6b3c7cb805854c5bc477c`, workspace manifests, and cached registry manifests read by Cargo 1.97.0.
- Resolution: `cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc`; start at the `relay` and `relayd` package IDs and follow normal and build dependency edges, excluding dev-only edges. Both binaries target the current Rust Windows MSVC build. No package was downloaded or compiled to make this inventory.
- The combined graph contains five workspace crates and 42 third-party crates from the crates.io registry. Thirty-seven third-party crates are reachable without a build edge; five are build-only (`cc`, `find-msvc-tools`, `pkg-config`, `shlex`, `vcpkg`). `serde_derive` is a proc macro used at build time even though Cargo represents its dependency edge as normal.
- This is a conservative *resolved package graph*, not a scan of bytes linked into either executable. A release build, packaging manifest, license-text collection, and native-component review must be compared against it before any binary distribution. The proposed synthetic preview runner and any packaged assets are outside this Rust-crate table.

The five workspace packages are `relay`, `relay-adapter`, `relay-contracts`, `relay-core`, and `relayd`, all version `0.1.0`. Their manifests currently report `MIT`; `LEGAL_LICENSING_AND_DISTRIBUTION.md` still leaves the product license decision open. That mismatch must be resolved by the owner before distribution. This inventory does not treat manifest metadata as a final licensing decision.

## Third-party resolved crates

The CLI/daemon column shows graph reachability for `relay.exe` / `relayd.exe`. A dash means the package is absent from that binary's selected normal/build dependency graph. License expressions are copied from Cargo metadata without normalization or legal interpretation.

| Crate | Version | Cargo license expression | CLI / daemon | Graph role |
| --- | --- | --- | --- | --- |
| bitflags | 2.13.2 | `MIT OR Apache-2.0` | — / yes | normal |
| block-buffer | 0.12.1 | `MIT OR Apache-2.0` | — / yes | normal |
| cc | 1.5.1 | `MIT OR Apache-2.0` | — / yes | build-only |
| cfg-if | 1.0.5 | `MIT OR Apache-2.0` | — / yes | normal |
| cpufeatures | 0.3.1 | `MIT OR Apache-2.0` | — / yes | normal |
| crypto-common | 0.2.2 | `MIT OR Apache-2.0` | — / yes | normal |
| digest | 0.11.3 | `MIT OR Apache-2.0` | — / yes | normal |
| fallible-iterator | 0.3.0 | `MIT/Apache-2.0` | — / yes | normal |
| fallible-streaming-iterator | 0.1.9 | `MIT/Apache-2.0` | — / yes | normal |
| find-msvc-tools | 0.1.14 | `MIT OR Apache-2.0` | — / yes | build-only |
| hybrid-array | 0.4.15 | `MIT OR Apache-2.0` | — / yes | normal |
| itoa | 1.0.18 | `MIT OR Apache-2.0` | yes / yes | normal |
| libc | 0.2.189 | `MIT OR Apache-2.0` | — / yes | normal |
| libsqlite3-sys | 0.38.2 | `MIT` | — / yes | normal |
| log | 0.4.34 | `MIT OR Apache-2.0` | — / yes | normal |
| memchr | 2.8.3 | `Unlicense OR MIT` | yes / yes | normal |
| notify | 8.2.0 | `CC0-1.0` | — / yes | normal |
| notify-types | 2.1.0 | `MIT OR Apache-2.0` | — / yes | normal |
| pkg-config | 0.3.34 | `MIT OR Apache-2.0` | — / yes | build-only |
| proc-macro2 | 1.0.107 | `MIT OR Apache-2.0` | yes / yes | normal; proc-macro support |
| quote | 1.0.47 | `MIT OR Apache-2.0` | yes / yes | normal; proc-macro support |
| rusqlite | 0.40.2 | `MIT` | — / yes | normal |
| same-file | 1.0.6 | `Unlicense/MIT` | — / yes | normal |
| serde | 1.0.229 | `MIT OR Apache-2.0` | yes / yes | normal |
| serde_core | 1.0.229 | `MIT OR Apache-2.0` | yes / yes | normal |
| serde_derive | 1.0.229 | `MIT OR Apache-2.0` | yes / yes | proc macro |
| serde_json | 1.0.151 | `MIT OR Apache-2.0` | yes / yes | normal |
| sha2 | 0.11.0 | `MIT OR Apache-2.0` | — / yes | normal |
| shlex | 2.0.1 | `MIT OR Apache-2.0` | — / yes | build-only |
| smallvec | 1.16.2 | `MIT OR Apache-2.0` | — / yes | normal |
| syn | 3.0.6 | `MIT OR Apache-2.0` | yes / yes | normal; proc-macro support |
| typenum | 1.20.1 | `MIT OR Apache-2.0` | — / yes | normal |
| unicode-ident | 1.0.26 | `(MIT OR Apache-2.0) AND Unicode-3.0` | yes / yes | normal; proc-macro support |
| vcpkg | 0.2.15 | `MIT/Apache-2.0` | — / yes | build-only |
| walkdir | 2.5.0 | `Unlicense/MIT` | — / yes | normal |
| winapi-util | 0.1.11 | `Unlicense OR MIT` | — / yes | normal |
| windows_x86_64_msvc | 0.53.1 | `MIT OR Apache-2.0` | — / yes | normal |
| windows-link | 0.2.1 | `MIT OR Apache-2.0` | yes / yes | normal |
| windows-sys | 0.60.2 | `MIT OR Apache-2.0` | — / yes | normal |
| windows-sys | 0.61.2 | `MIT OR Apache-2.0` | yes / yes | normal |
| windows-targets | 0.53.5 | `MIT OR Apache-2.0` | — / yes | normal |
| zmij | 1.0.23 | `MIT` | yes / yes | normal |

## Metadata coverage and human review queue

All 42 third-party entries have a nonempty Cargo `license` field; **unknown license expressions in this resolved graph: 0**. This does not verify the underlying license files, copyrights, embedded native sources, attribution, or which option of a dual-license expression the publisher will use. The expression counts are: 30 `MIT OR Apache-2.0`, three `MIT/Apache-2.0`, three `MIT`, two `Unlicense OR MIT`, two `Unlicense/MIT`, one `CC0-1.0`, and one `(MIT OR Apache-2.0) AND Unicode-3.0`.

Before packaging, a human release review must:

1. Resolve RELAY's own license decision and reconcile the five workspace manifests with that decision. Add the selected license text and contributor/release policy as needed.
2. Collect the actual license and copyright files for the exact crate versions, preserve required notices, and decide how alternative expressions are satisfied. The slash-form metadata (`MIT/Apache-2.0`, `Unlicense/MIT`) should be checked against each crate's own files; it is reproduced here exactly as metadata, not interpreted as SPDX syntax.
3. Review the combined `Unicode-3.0` expression for `unicode-ident`, `CC0-1.0` for `notify`, and Unlicense alternatives against the chosen notice bundle.
4. Inspect the bundled SQLite C source and its provenance separately from the `libsqlite3-sys` wrapper's `MIT` metadata. The workspace enables `rusqlite`'s `bundled` feature, so the wrapper expression alone does not describe every shipped source component.
5. Compare a finalized Windows release artifact and any packaged PowerShell runner, generated files, native DLLs, and third-party assets against this graph. Re-run the inventory whenever `Cargo.lock`, features, targets, or the packaged surface changes.

No notice obligation or redistribution permission is declared resolved by this inventory.
