# Solana Pinocchio AMM Workshop 🚀

欢迎来到 Solana 原生 AMM 开发工作坊！这个仓库包含了两个不同版本的 AMM 实现，旨在展示使用 **Pinocchio** 框架在不同阶段的架构演进与极致性能优化。

## 📂 目录结构

本仓库采用多项目组织方式，方便对比学习：

* **[`pinocchio_amm/`](./pinocchio_amm/)** (最新推荐 ✨)
  * **框架版本**: Pinocchio v0.10.x+
  * **核心特性**: 引入 `AccountView` 抽象，实现完全的**零拷贝 (Zero-copy)** 数据解析。
  * **优势**: 极致节省计算单元 (CU)，内存利用率达到峰值，是当前原生开发的最佳实践。

* **[`blueshift_native_amm/`](./blueshift_native_amm/)** (经典参考 📜)
  * **框架版本**: Pinocchio v0.9.x
  * **核心特性**: 基于 `AccountInfo` 的经典 Native 开发模式。
  * **意义**: 深入理解 Solana 账户模型的基础，是学习框架演进的必经之路。

---

## 🛠 技术栈

* **语言**: Rust (Edition 2024)
* **链上框架**: [Pinocchio](https://github.com/litesvm/pinocchio) (高性能、零依赖)
* **数学逻辑**: 恒定乘积公式 $x \cdot y = k$

## 🏆 挑战里程碑

本项目已通过 **Pinocchio AMM 开发者挑战**。

* [x] 成功实现 `Initialize` 指令并签署 PDA
* [x] 成功实现 `Swap` 逻辑与手续费计算
* [x] 解锁专属挑战 NFT 奖励 🏅

---

## 🚀 如何使用

1. **克隆仓库**:

```bash
git clone [https://github.com/qiaopengjun5162/solana-pinocchio-amm-workshop.git](https://github.com/qiaopengjun5162/solana-pinocchio-amm-workshop.git)
```

2. **进入对应目录**:
根据你想研究的版本进入 `pinocchio_amm` 或 `blueshift_native_amm`。
2. **编译**:

```bash
cargo build-sbf
```
