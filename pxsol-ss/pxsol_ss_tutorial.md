# Solana 程序最小骨架（pxsol-ss）创建与编译

本教程按 accu.cc 的流程整理，并补充当前工具链兼容性注意事项。

## 前置条件
- 已安装 Solana/Agave 工具链（含 `cargo-build-sbf`）
- edition2024 报错
### 第一方案
- 降低到2021版本
  ```bash
  cargo update -p constant_time_eq --precise 0.3.1
  ```
- 修改`Cargo.toml`文件
  ```bash
  edition = "2021"
  ```
### 第二种方案
- 如果使用稳定工具链（常见 rustc 1.84.1），可能触发 edition2024 报错。
  建议切到 edge/nightly：
  
  ```bash
  ~/.local/share/solana/install/active_release/bin/agave-install init edge
  ```
- 如果想切换回稳定版
  ```bash
  ~/.local/share/solana/install/active_release/bin/agave-install init stable
  ```




## 1. 创建项目
```bash
cargo new --lib pxsol-ss
cd pxsol-ss
```

## 2. 配置 Cargo.toml
编辑 `pxsol-ss/Cargo.toml`：
```toml
[package]
name = "pxsol-ss"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
solana-program = "=2.0.13"
```

说明：
- `cdylib` 用于生成可部署的 `.so`
- `lib` 方便本地测试
- `solana-program` 版本应与 SBF 工具链匹配（示例为 2.0.13）

## 3. 编写入口函数
编辑 `pxsol-ss/src/lib.rs`：
```rust
solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &solana_program::pubkey::Pubkey,
    accounts: &[solana_program::account_info::AccountInfo],
    data: &[u8],
) -> solana_program::entrypoint::ProgramResult {
    solana_program::msg!("Hello Solana!");
    Ok(())
}
```

## 4. 编译为 SBF
如果 PATH 里没有 `cargo-build-sbf`，用完整路径执行：
```bash
~/.local/share/solana/install/active_release/bin/cargo-build-sbf
```
或：
```bash
cargo build-sbf
```

## 5. 查看产物
```bash
ls target/deploy
```
应看到：
- `pxsol_ss.so`
- `pxsol_ss-keypair.json`

## 常见问题
1) `feature edition2024 is required`
   - 说明 SBF 工具链 rustc 太旧（常见 1.84.1）
   - 解决：切到 edge/nightly，或升级工具链

2) `no such command: build-sbf`
   - 未安装 Solana/Agave 工具链
   - 解决：安装并使用 `cargo-build-sbf`

3) `cfg` 或 unused 参数的警告
   - 可忽略；若想清掉警告，把参数名前加 `_` 即可
