// =============================================================================
// Make 指令 - Anchor 版本
// =============================================================================
// 本指令用于创建一个新的托管交易
// 创建者将代币 A 存入金库，并指定希望获得的代币 B 数量

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use crate::errors::EscrowError;
use crate::state::Escrow;

// =============================================================================
// Make 账户结构体
// =============================================================================
// 此结构体定义了 Make 指令所需的所有账户
//
// #[derive(Accounts)]:
//   Anchor 宏，用于自动实现账户验证和反序列化逻辑
//
// #[instruction(seed: u64)]:
//   声明此指令需要传入的参数（除了账户列表之外的参数）
//   这些参数可以在约束（constraints）中使用
#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Make<'info> {
    // ------------------------------------------------------------------------
    // 签名者账户
    // ------------------------------------------------------------------------
    // 创建者：发起托管交易的用户
    // - #[account(mut)]: 标记为可变账户（余额会被扣除）
    // - Signer: 要求此账户必须签名
    #[account(mut)]
    pub maker: Signer<'info>,

    // ------------------------------------------------------------------------
    // 托管账户（PDA）
    // ------------------------------------------------------------------------
    // 托管账户：存储交易状态的程序派生地址
    //
    // 约束说明：
    // - init: 创建新账户（如果已存在则失败）
    // - payer = maker: 由 maker 支付创建账户的租金（lamports）
    // - space: 账户数据空间大小
    //   - Escrow::INIT_SPACE: Escrow 结构体的大小（Anchor 自动计算）
    //   - Escrow::DISCRIMINATOR.len(): 8 字节的判别器（Anchor 用于类型识别）
    // - seeds: PDA 派生种子
    //   - b"escrow": 固定前缀字符串
    //   - maker.key().as_ref(): 创建者的公钥
    //   - seed.to_le_bytes().as_ref(): 随机种子（由客户端提供）
    // - bump: 自动验证并存储 bump 种子
    //
    // PDA 派生公式：
    //   PDA = PDA(["escrow", maker, seed], program_id)
    #[account(
        init,
        payer = maker,
        space = Escrow::INIT_SPACE + Escrow::DISCRIMINATOR.len(),
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,

    // ------------------------------------------------------------------------
    // 代币账户（Token Accounts）
    // ------------------------------------------------------------------------

    // 代币 A 的 Mint 账户：被存入金库的代币类型
    // - InterfaceAccount: 支持 Token Program 的不同接口版本
    // - mint::token_program = token_program: 验证 mint 账户由指定的 token_program 拥有
    #[account(
        mint::token_program = token_program
    )]
    pub mint_a: InterfaceAccount<'info, Mint>,

    // 代币 B 的 Mint 账户：创建者希望获得的代币类型
    #[account(
        mint::token_program = token_program
    )]
    pub mint_b: InterfaceAccount<'info, Mint>,

    // 创建者的代币 A 关联代币账户（ATA）
    // - mut: 可变（代币会被转出）
    // - associated_token::mint = mint_a: 验证此 ATA 的 mint 是 mint_a
    // - associated_token::authority = maker: 验证此 ATA 的所有者是 maker
    // - associated_token::token_program = token_program: 验证使用的 token program
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    // 金库账户（Vault）：存储被托管的代币 A
    // - init: 创建新的金库账户
    // - payer = maker: 由 maker 支付创建费用
    // - associated_token::authority = escrow: 金库由 escrow PDA 拥有（无私钥）
    //   这确保只有本程序能控制金库中的代币
    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    // ------------------------------------------------------------------------
    // 程序账户（Programs）
    // ------------------------------------------------------------------------
    // 这些程序账户用于 CPI（跨程序调用）

    // 关联代币程序：用于创建和管理 ATA
    pub associated_token_program: Program<'info, AssociatedToken>,

    // 代币程序：用于执行代币转账操作
    // - Interface: 支持不同的 Token Program 实现（Token Program 和 Token-2022）
    pub token_program: Interface<'info, TokenInterface>,

    // 系统程序：用于创建账户
    pub system_program: Program<'info, System>,
}

// =============================================================================
// Make 指令的方法实现
// =============================================================================
impl<'info> Make<'info> {
    // ------------------------------------------------------------------------
    // populate_escrow: 初始化托管账户数据
    // ------------------------------------------------------------------------
    // 将托管交易的参数写入到 escrow 账户中
    //
    // 参数：
    //   seed: PDA 派生种子（随机数）
    //   amount: 希望获得的代币 B 数量
    //   bump: PDA bump 种子（由 Anchor 自动计算）
    //
    // set_inner 方法：
    //   Anchor 提供的方法，用于设置账户的所有字段
    //   相当于一次性调用所有 setter 方法
    pub fn populate_escrow(&mut self,seed: u64,amount:u64,bump:u8) -> Result<()> {
        self.escrow.set_inner(Escrow {
            seed,
            maker:self.maker.key(),
            mint_a: self.mint_a.key(),
            mint_b: self.mint_b.key(),
            receive: amount,
            bump,
        });
        Ok(())
    }

    // ------------------------------------------------------------------------
    // deposit_token: 存入代币到金库
    // ------------------------------------------------------------------------
    // 将代币 A 从创建者的 ATA 转移到金库账户
    //
    // 参数：
    //   amount: 要存入的代币数量
    //
    // CPI（跨程序调用）说明：
    //   这里调用 Token Program 的 transfer_checked 指令
    //   transfer_checked 会验证：
    //   1. from 账户的余额是否足够
    //   2. mint 账户是否正确
    //   3. authority 是否签名（这里由 maker 签名）
    //
    // CpiContext::new:
    //   创建 CPI 上下文，包含：
    //   - 要调用的程序（token_program）
    //   - 所需的账户列表（TransferChecked 结构体）
    pub fn deposit_token(&self,amount:u64) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked{
                    from:self.maker_ata_a.to_account_info(),   // 从：创建者的 ATA
                    mint:self.mint_a.to_account_info(),        // mint 账户（用于验证代币类型）
                    to: self.vault.to_account_info(),          // 到：金库账户
                    authority:self.maker.to_account_info(),    // 权限：创建者必须签名
                },
            ),
            amount,              // 转账数量
            self.mint_a.decimals // 代币精度（用于验证数量格式）
        )?;
        Ok(())
    }
}

// =============================================================================
// Make 指令的 Handler 函数
// =============================================================================
// 这是 Make 指令的主处理函数，由 Anchor 框架自动调用
//
// 参数：
//   ctx: 上下文，包含所有账户和指令信息
//   seed: 随机种子，用于派生 PDA
//   receive: 希望获得的代币 B 数量
//   amount: 实际存入的代币 A 数量
//
// 返回值：
//   成功返回 Ok(())，失败返回 Err(...)
//
// ctx.bumps.escrow:
//   Anchor 自动计算的 PDA bump 值
//   在账户验证时，Anchor 会找到合适的 bump 并存储在这里
pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
    // ------------------------------------------------------------------------
    // 验证参数
    // ------------------------------------------------------------------------
    // require_gt!: Anchor 宏，验证数值大于 0
    // 如果验证失败，返回指定的错误
    require_gt!(receive, 0, EscrowError::InvalidAmount);
    require_gt!(amount, 0, EscrowError::InvalidAmount);

    // ------------------------------------------------------------------------
    // 初始化托管账户
    // ------------------------------------------------------------------------
    // 将托管交易的所有参数写入 escrow 账户
    ctx.accounts.populate_escrow(seed, receive, ctx.bumps.escrow)?;

    // ------------------------------------------------------------------------
    // 存入代币到金库
    // ------------------------------------------------------------------------
    // 将代币 A 从创建者账户转移到金库
    ctx.accounts.deposit_token(amount)?;

    Ok(())
}