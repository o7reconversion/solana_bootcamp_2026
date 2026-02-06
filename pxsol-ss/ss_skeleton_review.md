# 教程核查：Solana ss_skeleton
来源：https://accu.cc/content/solana/ss_skeleton/

## 可能问题
1) 依赖版本范围过宽，可能拉到需要 Rust 1.85 / edition2024 的新依赖。
   - 教程使用 `solana-program = "2"`，允许任意 2.x 版本。
   - 近期的传递依赖（例如 constant_time_eq 0.4.2）要求 edition 2024
     且 rustc 1.85。
   - `cargo build-sbf` 稳定工具链当前带的是 rustc 1.84.1，
     因此会报 "feature edition2024 is required"。

2) `-Znext-lockfile-bump` 的解释不准确/过时。
   - 该标志与 lockfile 格式兼容（Cargo lockfile v3）相关，
     而非“旧 solana_program 依赖旧 rustc”。
   - 这是 nightly-only 的 `-Z` 参数，稳定版可能直接报错。

3) 缺少 `cargo build-sbf` 的前置条件说明。
   - `cargo build-sbf` 来自 Solana/Agave 工具链，不是 Rust 自带命令。
   - 未安装时会提示 "no such command: build-sbf"。

## 建议修正
- 将 solana-program 固定到与 SBF 工具链匹配的版本。
  例如使用 solana-cli/agave 3.0.13 时，建议：
  `solana-program = "=2.0.13"`。
- 需要 edition2024 依赖时，改用 edge/nightly 的 SBF 工具链
  （rustc >= 1.85）。
- 说明 `cargo build-sbf` 使用自己的 rustc/cargo，
  可能与 PATH 中的 `cargo --version` 不同。
- `-Znext-lockfile-bump` 仅在 lockfile 兼容问题出现时再使用。
  如果看到 "the option `Z` is only accepted on the nightly compiler"，
  则移除该参数或改用 nightly SBF 工具链。

## 快速自检
- `cargo build-sbf --version` 应显示其 rustc 版本。
- 若报 `edition2024` 错误，则需要固定依赖或升级 SBF 工具链。
