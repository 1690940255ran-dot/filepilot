# 第三方组件与许可证

<!-- 本文件由 scripts/third-party-notices.py 生成，不要手改。 -->
<!-- 依赖变化后重跑：python scripts/third-party-notices.py -->

FilePilot 本身以 MIT 发布（见 `LICENSE`）。以下列出全部直接/传递依赖
及其许可证；各自的许可证全文见对应上游仓库。

## 1. 审计结论

- 共 665 个传递依赖，**未发现强 copyleft**（GPL/AGPL/SSPL 等）。
- 弱 copyleft（动态链接可用，静态链接需人工确认）：
  - [Rust] MIT OR Apache-2.0 OR LGPL-2.1-or-later：`r-efi 5.3.0`, `r-efi 6.0.0`
  - [Rust] MPL-2.0：`cssparser 0.36.0`, `cssparser-macros 0.6.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0`, `selectors 0.36.1`
  - [npm] MPL-2.0：`lightningcss 1.33.0`, `lightningcss-win32-x64-msvc 1.33.0`

### 弱 copyleft 的人工确认记录

- `r-efi` 标注 `MIT OR Apache-2.0 OR LGPL-2.1-or-later`——任选其一，
  本项目按 MIT/Apache-2.0 使用，不构成问题（2026-09-22 确认）。
- MPL-2.0（`cssparser`/`lightningcss` 等）是**文件级** copyleft：
  不修改这些依赖的源码时，按其原样链接分发即合规，
  源码在 crates.io / npm 公开可得（2026-09-22 确认）。

## 2. 不打包的内容

- **模型权重**：AI 模式的模型由用户自备（本地 Ollama 或云端 API），
  安装包不包含任何模型文件。
- **OCR 资源**：使用 Windows 系统自带的 WinRT OCR 与语言包，
  不随应用分发。
- **图标**：`src-tauri/icons/` 由 `scripts/make-icons.py` 生成，
  与项目同为 MIT。
- **仅在构建期使用的 npm 包**（例如 `ajv`）：契约校验器已经在构建期
  预编译成静态代码（见 `scripts/generate-validators.mjs`），运行时不加载
  这些包。下面第 4 节仍然把它们列出来——宁可多列，不要漏列。

## 3. Rust 依赖（Cargo.lock）

### (Apache-2.0 OR MIT) AND BSD-3-Clause（1 个）

- `encoding_rs` 0.8.41

### (MIT OR Apache-2.0) AND Unicode-3.0（1 个）

- `unicode-ident` 1.0.24

### 0BSD OR MIT OR Apache-2.0（1 个）

- `adler2` 2.0.1

### Apache-2.0（3 个）

- `sync_wrapper` 1.0.2
- `tao` 0.35.3
- `zopfli` 0.8.3

### Apache-2.0 / MIT（1 个）

- `fnv` 1.0.7

### Apache-2.0 AND MIT（1 个）

- `dpi` 0.1.2

### Apache-2.0 OR MIT（34 个）

- `atomic-waker` 1.1.2
- `autocfg` 1.5.1
- `bit-set` 0.8.0
- `bit-vec` 0.8.0
- `cargo_toml` 0.22.3
- `const-oid` 0.10.2
- `ctor` 0.8.0
- `ctor-proc-macro` 0.0.7
- `dtor` 0.3.0
- `dtor-proc-macro` 0.0.6
- `equivalent` 1.0.2
- `fastrand` 2.5.0
- `idna_adapter` 1.2.2
- `indexmap` 1.9.3
- `indexmap` 2.14.2
- `libappindicator` 0.9.0
- `libappindicator-sys` 0.9.0
- `muda` 0.19.3
- `multiversion_no_op` 1.0.0
- `pin-project-lite` 0.2.17
- `portable-atomic` 1.15.0
- `portable-atomic-util` 0.2.8
- `rustc-hash` 2.1.3
- `tauri` 2.11.5
- `tauri-build` 2.6.3
- `tauri-codegen` 2.6.3
- `tauri-macros` 2.6.3
- `tauri-runtime` 2.11.3
- `tauri-runtime-wry` 2.11.4
- `tauri-utils` 2.9.3
- `utf8_iter` 1.0.4
- `uuid` 1.26.1
- `window-vibrancy` 0.6.0
- `wry` 0.55.1

### Apache-2.0 WITH LLVM-exception（1 个）

- `target-lexicon` 0.12.16

### Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT（5 个）

- `linux-raw-sys` 0.12.1
- `rustix` 1.1.4
- `wasi` 0.11.1+wasi-snapshot-preview1
- `wasip2` 1.0.4+wasi-0.2.12
- `wit-bindgen` 0.57.1

### Apache-2.0/MIT（4 个）

- `cesu8` 1.1.0
- `dbus` 0.9.12
- `libdbus-sys` 0.2.7
- `pollster` 0.4.0

### BSD-3-Clause（2 个）

- `alloc-no-stdlib` 2.0.4
- `alloc-stdlib` 0.2.4

### BSD-3-Clause AND MIT（1 个）

- `brotli` 8.0.4

### BSD-3-Clause OR Apache-2.0（2 个）

- `moxcms` 0.8.1
- `pxfm` 0.1.30

### BSD-3-Clause OR MIT OR Apache-2.0（2 个）

- `num_enum` 0.7.6
- `num_enum_derive` 0.7.6

### BSD-3-Clause/MIT（1 个）

- `brotli-decompressor` 5.0.3

### CC0-1.0 OR MIT-0 OR Apache-2.0（1 个）

- `dunce` 1.0.5

### ISC（1 个）

- `libloading` 0.7.4

### MIT（117 个）

- `atk` 0.18.2
- `atk-sys` 0.18.2
- `block2` 0.6.2
- `bytes` 1.12.1
- `cairo-rs` 0.18.5
- `cairo-sys-rs` 0.18.2
- `cargo_metadata` 0.19.2
- `cfb` 0.7.3
- `combine` 4.6.8
- `darling` 0.24.1
- `darling_core` 0.24.1
- `darling_macro` 0.24.1
- `derive_more` 2.1.1
- `derive_more-impl` 2.1.1
- `dlib` 0.5.3
- `dlopen2` 0.8.2
- `dlopen2_derive` 0.4.3
- `dom_query` 0.27.0
- `embed-resource` 3.0.11
- `gdk` 0.18.2
- `gdk-pixbuf` 0.18.5
- `gdk-pixbuf-sys` 0.18.0
- `gdk-sys` 0.18.2
- `gdkwayland-sys` 0.18.2
- `gdkx11` 0.18.2
- `gdkx11-sys` 0.18.2
- `generic-array` 0.14.7
- `gio` 0.18.4
- `gio-sys` 0.18.1
- `glib` 0.18.5
- `glib-macros` 0.18.5
- `glib-sys` 0.18.1
- `gobject-sys` 0.18.0
- `gtk` 0.18.2
- `gtk-sys` 0.18.2
- `gtk3-macros` 0.18.2
- `http-body` 1.1.0
- `http-body-util` 0.1.5
- `hyper` 1.11.1
- `hyper-util` 0.1.20
- `ico` 0.5.0
- `infer` 0.19.0
- `javascriptcore-rs` 1.1.2
- `javascriptcore-rs-sys` 1.1.1
- `libredox` 0.1.24
- `libsqlite3-sys` 0.38.2
- `lopdf` 0.45.0
- `memoffset` 0.9.1
- `mio` 1.2.3
- `new_debug_unreachable` 1.0.6
- `nom` 8.0.0
- `objc2` 0.6.4
- `objc2-encode` 4.1.0
- `objc2-foundation` 0.3.2
- `pango` 0.18.3
- `pango-sys` 0.18.0
- `phf` 0.13.1
- `phf_codegen` 0.13.1
- `phf_generator` 0.13.1
- `phf_macros` 0.13.1
- `phf_shared` 0.13.1
- `plist` 1.10.1
- `precomputed-hash` 0.1.1
- `quick-xml` 0.41.0
- `quick-xml` 0.42.0
- `redox_syscall` 0.5.18
- `redox_users` 0.5.2
- `rfd` 0.17.2
- `rsqlite-vfs` 0.1.1
- `rusqlite` 0.40.2
- `schemars` 0.8.22
- `schemars` 0.9.0
- `schemars` 1.2.2
- `schemars_derive` 0.8.22
- `schemars_derive` 1.2.2
- `simd-adler32` 0.3.10
- `slab` 0.4.12
- `soup3` 0.5.0
- `soup3-sys` 0.5.0
- `sqlite-wasm-rs` 0.5.5
- `strsim` 0.11.1
- `synstructure` 0.14.0
- `tauri-winres` 0.3.6
- `tokio` 1.53.1
- `tokio-util` 0.7.19
- `tower` 0.5.3
- `tower-http` 0.6.11
- `tower-layer` 0.3.3
- `tower-service` 0.3.3
- `tracing` 0.1.44
- `tracing-core` 0.1.36
- `try-lock` 0.2.5
- `ts-rs` 12.0.1
- `ts-rs-macros` 12.0.1
- `urlpattern` 0.3.0
- `version-compare` 0.2.1
- `vswhom` 0.1.0
- `vswhom-sys` 0.1.3
- `want` 0.3.1
- `wayland-backend` 0.3.17
- `wayland-client` 0.31.15
- `wayland-protocols` 0.32.13
- `wayland-scanner` 0.31.11
- `wayland-sys` 0.31.11
- `webkit2gtk` 2.0.2
- `webkit2gtk-sys` 2.0.2
- `webview2-com` 0.38.2
- `webview2-com-macros` 0.8.1
- `webview2-com-sys` 0.38.2
- `winnow` 0.5.40
- `winnow` 0.7.15
- `winnow` 1.0.4
- `winreg` 0.55.0
- `x11` 2.21.0
- `x11-dl` 2.21.0
- `zip` 8.6.0
- `zmij` 1.0.23

### MIT OR Apache-2.0（242 个）

- `aes` 0.9.3
- `android_system_properties` 0.1.6
- `anyhow` 1.0.104
- `base64` 0.21.7
- `base64` 0.22.1
- `base64` 0.23.1
- `bitflags` 2.13.2
- `block-buffer` 0.10.4
- `block-buffer` 0.12.1
- `block-padding` 0.4.2
- `bumpalo` 3.20.3
- `camino` 1.2.6
- `cargo-platform` 0.1.9
- `cbc` 0.2.1
- `cc` 1.4.6
- `cfg-expr` 0.15.8
- `cfg-if` 1.0.4
- `chacha20` 0.10.2
- `chrono` 0.4.45
- `cipher` 0.5.2
- `cookie` 0.18.2
- `core-foundation` 0.10.1
- `core-foundation-sys` 0.8.7
- `core-graphics` 0.25.0
- `core-graphics-types` 0.2.0
- `cpubits` 0.1.1
- `cpufeatures` 0.2.17
- `cpufeatures` 0.3.1
- `crc32fast` 1.5.2
- `crossbeam-channel` 0.5.17
- `crossbeam-deque` 0.8.8
- `crossbeam-epoch` 0.9.21
- `crossbeam-utils` 0.8.23
- `crypto-common` 0.1.7
- `crypto-common` 0.2.2
- `defmt` 1.1.1
- `defmt-macros` 1.1.1
- `defmt-parser` 1.0.0
- `deranged` 0.5.8
- `digest` 0.10.7
- `digest` 0.11.3
- `dirs` 6.0.0
- `dirs-sys` 0.5.0
- `displaydoc` 0.2.7
- `dtoa` 1.0.11
- `dyn-clone` 1.0.20
- `ecb` 0.2.1
- `either` 1.18.0
- `embed_plist` 1.2.2
- `erased-serde` 0.4.10
- `errno` 0.3.14
- `fdeflate` 0.3.7
- `field-offset` 0.3.6
- `find-msvc-tools` 0.1.12
- `flate2` 1.1.10
- `form_urlencoded` 1.2.2
- `futures-channel` 0.3.34
- `futures-core` 0.3.34
- `futures-executor` 0.3.34
- `futures-io` 0.3.34
- `futures-macro` 0.3.34
- `futures-sink` 0.3.34
- `futures-task` 0.3.34
- `futures-util` 0.3.34
- `getrandom` 0.2.17
- `getrandom` 0.3.4
- `getrandom` 0.4.3
- `glob` 0.3.4
- `hashbrown` 0.12.3
- `hashbrown` 0.16.1
- `hashbrown` 0.17.1
- `hashlink` 0.12.2
- `heck` 0.4.1
- `heck` 0.5.0
- `hex` 0.4.3
- `html5ever` 0.38.0
- `http` 1.5.0
- `httparse` 1.10.1
- `hybrid-array` 0.4.15
- `iana-time-zone` 0.1.65
- `iana-time-zone-haiku` 0.1.2
- `idna` 1.1.0
- `image` 0.25.10
- `inout` 0.2.2
- `ipnet` 2.12.2
- `itoa` 1.0.18
- `jni-sys` 0.3.1
- `jni-sys` 0.4.1
- `jni-sys-macros` 0.4.1
- `js-sys` 0.3.105
- `jsonptr` 0.6.3
- `keyboard-types` 0.7.0
- `libc` 0.2.189
- `lock_api` 0.4.14
- `log` 0.4.34
- `markup5ever` 0.38.0
- `md-5` 0.11.0
- `mime` 0.3.17
- `multiversion` 0.9.0
- `multiversion-macros` 0.9.0
- `ndk` 0.9.0
- `ndk-sys` 0.6.0+11769913
- `num-conv` 0.2.2
- `num-traits` 0.2.19
- `once_cell` 1.21.4
- `parking_lot` 0.12.5
- `parking_lot_core` 0.9.12
- `percent-encoding` 2.3.2
- `pkg-config` 0.3.34
- `png` 0.17.16
- `png` 0.18.1
- `powerfmt` 0.2.0
- `proc-macro-crate` 1.3.1
- `proc-macro-crate` 2.0.2
- `proc-macro-crate` 3.5.0
- `proc-macro-error` 1.0.4
- `proc-macro-error-attr` 1.0.4
- `proc-macro2` 1.0.107
- `quote` 1.0.47
- `rand` 0.10.2
- `rand_core` 0.10.1
- `rayon` 1.12.0
- `rayon-core` 1.13.0
- `ref-cast` 1.0.27
- `ref-cast-impl` 1.0.27
- `regex` 1.13.1
- `regex-automata` 0.4.18
- `regex-syntax` 0.8.11
- `reqwest` 0.13.5
- `rustc_version` 0.4.1
- `rustversion` 1.0.23
- `scopeguard` 1.2.0
- `semver` 1.0.28
- `serde` 1.0.229
- `serde-untagged` 0.1.9
- `serde_core` 1.0.229
- `serde_derive` 1.0.229
- `serde_derive_internals` 0.29.1
- `serde_derive_internals` 0.30.0
- `serde_json` 1.0.151
- `serde_repr` 0.1.21
- `serde_spanned` 0.6.9
- `serde_spanned` 1.1.1
- `serde_with` 3.23.0
- `serde_with_macros` 3.23.0
- `serialize-to-javascript` 0.1.2
- `serialize-to-javascript-impl` 0.1.2
- `servo_arc` 0.4.3
- `sha2` 0.10.9
- `sha2` 0.11.0
- `shlex` 2.0.1
- `simdutf8` 0.1.5
- `smallvec` 1.16.1
- `socket2` 0.6.5
- `softbuffer` 0.4.8
- `stable_deref_trait` 1.2.1
- `string_cache` 0.9.0
- `string_cache_codegen` 0.6.1
- `swift-rs` 1.0.8
- `syn` 1.0.109
- `syn` 2.0.119
- `syn` 3.0.5
- `system-deps` 6.2.2
- `tao-macros` 0.1.4
- `tempfile` 3.27.0
- `tendril` 0.5.1
- `thiserror` 1.0.69
- `thiserror` 2.0.20
- `thiserror-impl` 1.0.69
- `thiserror-impl` 2.0.20
- `time` 0.3.55
- `time-core` 0.1.9
- `time-macros` 0.2.32
- `toml` 0.8.2
- `toml` 0.9.12+spec-1.1.0
- `toml` 1.1.6+spec-1.1.0
- `toml_datetime` 0.6.3
- `toml_datetime` 0.7.5+spec-1.1.0
- `toml_datetime` 1.1.1+spec-1.1.0
- `toml_edit` 0.19.15
- `toml_edit` 0.20.2
- `toml_edit` 0.25.15+spec-1.1.0
- `toml_parser` 1.1.3+spec-1.1.0
- `toml_writer` 1.1.2+spec-1.1.0
- `tray-icon` 0.24.2
- `typed-path` 0.12.3
- `typeid` 1.0.3
- `typenum` 1.20.1
- `unicode-bidi` 0.3.18
- `unicode-normalization` 0.1.25
- `unicode-segmentation` 1.13.3
- `url` 2.5.8
- `wasm-bindgen` 0.2.128
- `wasm-bindgen-futures` 0.4.78
- `wasm-bindgen-macro` 0.2.128
- `wasm-bindgen-macro-support` 0.2.128
- `wasm-bindgen-shared` 0.2.128
- `wasm-streams` 0.5.0
- `web-sys` 0.3.105
- `web_atoms` 0.2.6
- `weezl` 0.2.1
- `windows` 0.61.3
- `windows` 0.62.2
- `windows-collections` 0.2.0
- `windows-collections` 0.3.2
- `windows-core` 0.61.2
- `windows-core` 0.62.2
- `windows-future` 0.2.1
- `windows-future` 0.3.2
- `windows-implement` 0.60.2
- `windows-interface` 0.59.3
- `windows-link` 0.1.3
- `windows-link` 0.2.1
- `windows-numerics` 0.2.0
- `windows-numerics` 0.3.1
- `windows-result` 0.3.4
- `windows-result` 0.4.1
- `windows-strings` 0.4.2
- `windows-strings` 0.5.1
- `windows-sys` 0.45.0
- `windows-sys` 0.59.0
- `windows-sys` 0.61.2
- `windows-targets` 0.42.2
- `windows-targets` 0.52.6
- `windows-threading` 0.1.0
- `windows-threading` 0.2.1
- `windows-version` 0.1.7
- `windows_aarch64_gnullvm` 0.42.2
- `windows_aarch64_gnullvm` 0.52.6
- `windows_aarch64_msvc` 0.42.2
- `windows_aarch64_msvc` 0.52.6
- `windows_i686_gnu` 0.42.2
- `windows_i686_gnu` 0.52.6
- `windows_i686_gnullvm` 0.52.6
- `windows_i686_msvc` 0.42.2
- `windows_i686_msvc` 0.52.6
- `windows_x86_64_gnu` 0.42.2
- `windows_x86_64_gnu` 0.52.6
- `windows_x86_64_gnullvm` 0.42.2
- `windows_x86_64_gnullvm` 0.52.6
- `windows_x86_64_msvc` 0.42.2
- `windows_x86_64_msvc` 0.52.6

### MIT OR Apache-2.0 OR LGPL-2.1-or-later（2 个）

- `r-efi` 5.3.0
- `r-efi` 6.0.0

### MIT OR Apache-2.0 OR Zlib（3 个）

- `raw-window-handle` 0.6.2
- `zune-core` 0.5.3
- `zune-jpeg` 0.5.15

### MIT OR Zlib OR Apache-2.0（2 个）

- `miniz_oxide` 0.8.9
- `miniz_oxide` 0.9.1

### MIT/Apache-2.0（27 个）

- `bitflags` 1.3.2
- `bs58` 0.5.1
- `core_detect` 1.0.0
- `downcast-rs` 1.2.1
- `fallible-iterator` 0.3.0
- `fallible-streaming-iterator` 0.1.9
- `foreign-types` 0.5.0
- `foreign-types-macros` 0.2.4
- `foreign-types-shared` 0.3.1
- `ident_case` 1.0.1
- `jni` 0.21.1
- `json-patch` 3.0.1
- `rangemap` 1.8.0
- `scoped-tls` 1.0.1
- `siphasher` 1.0.3
- `stringprep` 0.1.5
- `unic-char-property` 0.9.0
- `unic-char-range` 0.9.0
- `unic-common` 0.9.0
- `unic-ucd-ident` 0.9.0
- `unic-ucd-version` 0.9.0
- `unicode-properties` 0.1.4
- `vcpkg` 0.2.15
- `version_check` 0.9.5
- `winapi` 0.3.9
- `winapi-i686-pc-windows-gnu` 0.4.0
- `winapi-x86_64-pc-windows-gnu` 0.4.0

### MPL-2.0（5 个）

- `cssparser` 0.36.0
- `cssparser-macros` 0.6.1
- `dtoa-short` 0.3.5
- `option-ext` 0.2.0
- `selectors` 0.36.1

### Unicode-3.0（18 个）

- `icu_collections` 2.3.0
- `icu_locale_core` 2.3.0
- `icu_normalizer` 2.3.0
- `icu_normalizer_data` 2.3.0
- `icu_properties` 2.3.0
- `icu_properties_data` 2.3.0
- `icu_provider` 2.3.1
- `litemap` 0.8.3
- `potential_utf` 0.1.6
- `tinystr` 0.8.4
- `writeable` 0.6.4
- `yoke` 0.8.3
- `yoke-derive` 0.8.3
- `zerofrom` 0.1.8
- `zerofrom-derive` 0.1.8
- `zerotrie` 0.2.5
- `zerovec` 0.11.8
- `zerovec-derive` 0.11.6

### Unlicense OR MIT（11 个）

- `aho-corasick` 1.1.5
- `byteorder` 1.5.0
- `byteorder-lite` 0.1.0
- `jiff` 0.2.37
- `jiff-core` 0.1.1
- `jiff-static` 0.2.37
- `jiff-tzdb` 0.1.8
- `jiff-tzdb-platform` 0.1.3
- `memchr` 2.8.3
- `termcolor` 1.4.1
- `winapi-util` 0.1.11

### Unlicense/MIT（2 个）

- `same-file` 1.0.6
- `walkdir` 2.5.0

### Zlib（2 个）

- `foldhash` 0.2.0
- `zlib-rs` 0.6.8

### Zlib OR Apache-2.0 OR MIT（17 个）

- `bytemuck` 1.25.2
- `dispatch2` 0.3.1
- `objc2-app-kit` 0.3.2
- `objc2-cloud-kit` 0.3.2
- `objc2-core-data` 0.3.2
- `objc2-core-foundation` 0.3.2
- `objc2-core-graphics` 0.3.2
- `objc2-core-image` 0.3.2
- `objc2-core-location` 0.3.2
- `objc2-core-text` 0.3.2
- `objc2-exception-helper` 0.1.1
- `objc2-io-surface` 0.3.2
- `objc2-quartz-core` 0.3.2
- `objc2-ui-kit` 0.3.2
- `objc2-user-notifications` 0.3.2
- `objc2-web-kit` 0.3.2
- `tinyvec` 1.13.3

## 4. npm 依赖（pnpm-lock.yaml）

### Apache-2.0（9 个）

- `aria-query` 5.3.0
- `baseline-browser-mapping` 2.11.24
- `detect-libc` 2.1.2
- `eslint-visitor-keys` 3.4.3
- `expect-type` 1.4.0
- `playwright` 1.63.0
- `playwright-core` 1.63.0
- `typescript` 6.0.3
- `xml-name-validator` 5.0.0

### BSD-2-Clause（8 个）

- `entities` 8.1.0
- `eslint-scope` 9.1.2
- `espree` 11.2.0
- `esrecurse` 4.3.0
- `estraverse` 5.3.0
- `esutils` 2.0.3
- `uri-js` 4.4.1
- `webidl-conversions` 8.0.1

### BSD-3-Clause（4 个）

- `esquery` 1.7.0
- `fast-uri` 3.1.8
- `source-map-js` 1.2.1
- `tough-cookie` 6.0.2

### BlueOak-1.0.0（2 个）

- `lru-cache` 11.5.2
- `minimatch` 10.2.6

### CC-BY-4.0（1 个）

- `caniuse-lite` 1.0.30001810

### CC0-1.0（1 个）

- `mdn-data` 2.27.1

### ISC（10 个）

- `electron-to-chromium` 1.5.430
- `flatted` 3.4.4
- `glob-parent` 6.0.2
- `isexe` 2.0.0
- `picocolors` 1.1.1
- `saxes` 6.0.0
- `semver` 6.3.1
- `siginfo` 2.0.0
- `which` 2.0.2
- `yallist` 3.1.1

### MIT（118 个）

- `acorn` 8.18.0
- `acorn-jsx` 5.3.2
- `ajv` 6.15.0
- `ansi-regex` 5.0.1
- `ansi-styles` 5.2.0
- `assertion-error` 2.0.1
- `ast-v8-to-istanbul` 1.0.6
- `balanced-match` 4.0.4
- `bidi-js` 1.1.0
- `brace-expansion` 5.0.12
- `browserslist` 4.29.0
- `cacheable` 2.5.0
- `chai` 6.2.2
- `convert-source-map` 2.0.0
- `cross-spawn` 7.0.6
- `css-tree` 3.2.1
- `css.escape` 1.5.1
- `csstype` 3.2.3
- `data-urls` 7.0.0
- `debug` 4.4.3
- `decimal.js` 10.6.0
- `deep-is` 0.1.4
- `dequal` 2.0.3
- `dom-accessibility-api` 0.5.16
- `es-module-lexer` 2.3.2
- `escalade` 3.2.0
- `escape-string-regexp` 4.0.0
- `eslint` 10.10.0
- `eslint-plugin-react-hooks` 7.1.1
- `eslint-plugin-react-refresh` 0.5.7
- `estree-walker` 3.0.3
- `fast-deep-equal` 3.1.3
- `fast-json-stable-stringify` 2.1.0
- `fast-levenshtein` 2.0.6
- `fdir` 6.5.0
- `file-entry-cache` 11.1.5
- `find-up` 5.0.0
- `flat-cache` 6.1.23
- `gensync` 1.0.0-beta.2
- `globals` 17.12.0
- `hashery` 1.5.1
- `hermes-estree` 0.25.1
- `hermes-parser` 0.25.1
- `hookified` 1.15.1
- `html-encoding-sniffer` 6.0.0
- `ignore` 7.0.9
- `imurmurhash` 0.1.4
- `indent-string` 4.0.0
- `is-extglob` 2.1.1
- `is-glob` 4.0.3
- `is-potential-custom-element-name` 1.0.1
- `js-tokens` 4.0.0
- `jsdom` 30.0.1
- `jsesc` 3.1.0
- `json-schema-traverse` 0.4.1
- `json-stable-stringify-without-jsonify` 1.0.1
- `json5` 2.2.3
- `keyv` 5.6.0
- `levn` 0.4.1
- `locate-path` 6.0.0
- `lz-string` 1.5.0
- `magic-string` 1.4.1
- `magicast` 0.5.5
- `min-indent` 1.0.1
- `ms` 2.1.3
- `nanoid` 3.3.19
- `natural-compare` 1.4.0
- `node-releases` 2.0.55
- `obug` 2.2.1
- `optionator` 0.9.4
- `p-limit` 3.1.0
- `p-locate` 5.0.0
- `parse5` 8.0.1
- `path-exists` 4.0.0
- `path-key` 3.1.1
- `picomatch` 4.0.7
- `postcss` 8.5.28
- `prelude-ls` 1.2.1
- `pretty-format` 27.5.1
- `punycode` 2.3.1
- `qified` 0.10.1
- `react` 19.3.0
- `react-dom` 19.3.0
- `react-is` 17.0.2
- `redent` 3.0.0
- `require-from-string` 2.0.2
- `rolldown` 1.2.8
- `scheduler` 0.28.0
- `shebang-command` 2.0.0
- `shebang-regex` 3.0.0
- `stackback` 0.0.2
- `std-env` 4.2.0
- `strip-indent` 3.0.0
- `symbol-tree` 3.2.4
- `tinybench` 6.1.4
- `tinyexec` 1.3.0
- `tinyglobby` 0.2.17
- `tinyrainbow` 3.1.1
- `tldts` 7.4.13
- `tldts-core` 7.4.13
- `tr46` 6.0.0
- `ts-api-utils` 2.5.0
- `type-check` 0.4.0
- `typescript-eslint` 8.70.0
- `undici` 8.10.2
- `undici-types` 6.21.0
- `update-browserslist-db` 1.3.3
- `vite` 8.3.0
- `vitest` 5.0.1
- `w3c-xmlserializer` 5.0.0
- `whatwg-mimetype` 5.0.0
- `whatwg-url` 16.0.1
- `why-is-node-running` 2.3.0
- `word-wrap` 1.2.5
- `xmlchars` 2.2.0
- `yocto-queue` 0.1.0
- `zod` 4.6.5
- `zod-validation-error` 4.0.2

### MPL-2.0（2 个）

- `lightningcss` 1.33.0
- `lightningcss-win32-x64-msvc` 1.33.0
