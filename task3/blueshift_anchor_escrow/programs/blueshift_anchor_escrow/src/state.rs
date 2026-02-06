use anchor_lang::prelude::*; 
// PDA 种子常量与托管账户状态。 
pub const ESCROW_SEED: &[u8] = b"escrow"; // 用于派生 escrow PDA 的静态种子。
// Escrow 账户数据结构。 // 账户结构体章节标题。
#[derive(InitSpace)] // 自动计算账户大小用于租金豁免。
#[account(discriminator = 1)] // 使用自定义账户鉴别器值 1。
pub struct Escrow { // Escrow 链上状态开始。
    pub seed: u64, // 随机种子，允许同一 maker 多次创建托管。
    pub maker: Pubkey, // 创建托管的 maker 公钥。
    pub mint_a: Pubkey, // maker 提供的 Token A 的 Mint。
    pub mint_b: Pubkey, // maker 期望获得的 Token B 的 Mint。
    pub receive: u64, // maker 期望接收的 Token B 数量。
    pub bump: u8, // PDA bump，用于派生 escrow 地址。
} // Escrow 结构体结束
