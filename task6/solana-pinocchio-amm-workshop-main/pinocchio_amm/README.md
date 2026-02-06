
# Pinocchio AMM: 极致性能的 Solana 原生自动做市商

这是一个基于 **Pinocchio** 框架构建的 Solana 云原生 AMM 协议。本项目旨在展示如何通过零拷贝（Zero-copy）和最小化指令开销，在 Solana 上实现极致的计算单元（CU）优化。

## 🚀 项目亮点

* **零成本抽象**：放弃传统的 `Borsh` 序列化，直接操作账户内存快照。
* **计算单元（CU）优化**：由于移除了繁重的依赖包，`Initialize` 和 `Swap` 指令的 CU 消耗远低于传统的 Anchor 框架。
* **内存对齐安全**：在处理 `packed` 结构体时，通过 `read_unaligned` 和 `unsafe` 指针操作，确保了 BPF 环境下的执行安全。
* **双版本演进**：
* **Old Version (`blueshift_native_amm`)**: 基于 `AccountInfo` 模式，适合理解经典的 Solana 账户模型。
* **New Version (`pinocchio_amm`)**: 升级至最新的 `AccountView` 抽象，完全释放了零拷贝的潜力。

---

## 📂 项目结构

```bash
src/
├── lib.rs            # 程序入口，指令分发（Dispatch）中心
├── state.rs          # 核心状态定义：Config 与数据布局
├── instructions/     # 指令逻辑实现
│   ├── mod.rs        # 模块化导出
│   ├── initialize.rs # 初始化 AMM：创建 PDA、设置权限
│   ├── deposit.rs    # 注入流动性：铸造 LP 代币
│   ├── withdraw.rs   # 销毁流动性：提取底层资产
│   └── swap.rs       # 代币交换：基于恒定乘积公式 (x * y = k)
└── curve.rs          # (可选) 外部参考的数学公式逻辑

```

---

## 🛠 技术深度解析

### 1. 内存管理与 `unsafe` 的艺术

在本项目中，`unsafe` 并不代表“危险”，而是代表“**手动验证的安全**”。
为了跳过数据拷贝，我们直接将账户数据映射到 Rust 结构体中：

```rust
// 通过 read_unaligned 处理非对齐内存
let auth = unsafe { core::ptr::addr_of!(self.authority).read_unaligned() };

```

### 2. 双版本差异对比

| 特性 | 旧版本 (v0.9.x) | 新版本 (v0.10.x+) |
| --- | --- | --- |
| **核心抽象** | `AccountInfo` | `AccountView` |
| **错误处理** | `minimum_balance` | `try_minimum_balance` (防溢出) |
| **数据访问** | `borrow_unchecked` | 更安全的 `Ref` 封装 |
| **性能** | 极高性能 | 巅峰性能 (最小化栈空间消耗) |

---

## 💡 指令流程说明

### `Initialize`

1. 创建协议 `Config` 账户（PDA）。
2. 创建 `Mint LP` 代币账户。
3. 将 `Mint LP` 的铸币权锁定给 `Config` 账户，建立权限闭环。

### `Swap`

采用恒定乘积公式 。

1. 验证交易对账户。
2. 计算输入金额扣除手续费后的产出。
3. 执行转移操作并更新账户状态。

---

## 🔨 开发与构建

### 环境要求

* Rust 1.75+ (推荐使用 2024 Edition)
* Solana CLI 1.18+
* Pinocchio Framework

### 构建指令

```bash
# 构建 BPF 程序
cargo build-sbf

# 运行测试 (如果有测试脚本)
cargo test-sbf

```

---

## 📝 教学参考说明

本项目在 `docs/reference` 目录下（或外部依赖中）参考了 `constant-product-curve`。这主要用于教学演示如何在不引入臃肿依赖的情况下，手动实现复杂的金融数学逻辑。

---
