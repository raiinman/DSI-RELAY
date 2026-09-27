# Third-party notice and bundled SQLite source review

Status: technical review of the Windows measurement-preview graph locked by `Cargo.lock` SHA-256 `8f46c4a561e2716ffb43d0a9ade13979d9ee021147a6b3c7cb805854c5bc477c`, 2026-09-26. This is not legal distribution approval.

## Resolved crate graph and retained texts

The CLI/daemon graph selected by `cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc`, following normal and build edges but excluding dev-only edges, contains 42 registry crates. The preview inventory names every version and expression. The package contains 86 original license/copyright files from those exact cached crate source directories. A byte comparison found zero differences between source files and packaged copies. No source-tree file named `NOTICE` was found in the selected crates; the only additional nested license candidate was `libsqlite3-sys/sqlcipher/LICENSE`, and the selected build features do not enable SQLCipher. Build-only crates are retained conservatively in the inventory.

The package records these choices while retaining every available original license text:

| Cargo expression | Crates | Recorded choice | Retained special text |
| --- | ---: | --- | --- |
| `MIT OR Apache-2.0` | 30 | MIT | Both MIT and Apache texts |
| `MIT/Apache-2.0` | 3 | MIT | Both MIT and Apache texts |
| `MIT` | 3 | MIT | MIT text |
| `Unlicense OR MIT` | 2 | MIT | MIT, Unlicense, and COPYING |
| `Unlicense/MIT` | 2 | MIT | MIT, Unlicense, and COPYING |
| `CC0-1.0` | 1 (`notify`) | CC0-1.0 | Full CC0 text |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 (`unicode-ident`) | MIT AND Unicode-3.0 | MIT, Apache, and full Unicode V3 text |

The slash-form crate metadata is copied as published. The cached `fallible-streaming-iterator`, `same-file`, `walkdir`, `memchr`, and `winapi-util` READMEs explicitly describe their alternatives; `vcpkg` points to its MIT and Apache files; `fallible-iterator` publishes both license files and the slash-form expression in its own manifest. For the reviewed graph, the packager pins a SHA-256 fingerprint over crate name, version, expression, recorded choice, path, and bytes of every copied license file: `efbcc57bf0ec67fc9a91d682ac06080be04c6b0800c069a9ddaf2acb828532f5`. A lockfile, crate count, expression, choice, file name, or file-byte change reopens `THIRD_PARTY_NOTICE_SELECTION_REVIEW` in the bundle manifest.

This check covers named license/notice files and package metadata. It does not audit every source comment, decide jurisdictional validity, or approve publication. The bundle retains `LEGAL_RELEASE_APPROVAL_PENDING`.

## Bundled SQLite provenance

The selected `libsqlite3-sys` crate is version `0.38.2`; its resolved features include `bundled` and exclude `sqlcipher` and `bundled-sqlcipher`. Its build script compiles `sqlite3/sqlite3.c`. The cached header identifies SQLite `3.53.2` and `SQLITE_SOURCE_ID` `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`, matching [SQLite's release history](https://www.sqlite.org/changes.html). The cached `sqlite3.c` SHA3-256 is `44fd61b9f93b4155105cb2d80c957ae6c64a8b5bd6ed51a4992f0dbd438e4e11`, exactly the digest SQLite publishes for 3.53.2. The packager pins that reviewed file by SHA-256 `0a409f1633283fa31a9126b11fbfd64a1991c5d30defad07e5745d4667f5e23d` and the matching header by SHA-256 `9e69a1353a4288450b0d5239ede11fc7f1f4c8e5eb07491fc8317eacb5b7de7e`.

[SQLite states](https://www.sqlite.org/copyright.html) that its deliverable code is dedicated to the public domain and can be distributed in compiled form. Its [amalgamation documentation](https://www.sqlite.org/amalgamation.html) describes `sqlite3.c` as the source used to embed the SQLite library. The package includes a `SQLITE-PROVENANCE.txt` record; it does not bundle a separate SQLite DLL. The wrapper crate's MIT license remains in `third-party/libsqlite3-sys-0.38.2/LICENSE`. If the crate version, source bytes, lockfile, or bundled/SQLCipher feature choice changes, the manifest reopens `BUNDLED_SQLITE_PROVENANCE_REVIEW`.

The technical matches above clear those two manifest checks for this exact graph. Clean-account smoke, legal release approval, and publisher trust/security review remain open.
