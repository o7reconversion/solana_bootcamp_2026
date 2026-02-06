// =============================================================================
// Take 指令 - Anchor 版本
// =============================================================================
// 本指令用于接受一个现有的托管交易
// 接受者向创建者发送代币 B，并从金库中获得代币 A
//
// 执行流程：
// 1. 验证托管账户的有效性
// 2. 接受者向创建者发送指定数量的代币 B
// 3. 从金库中将代币 A 转移给接受者
// 4. 关闭金库账户，将剩余 lamports 返还给创建者
// 5. 关闭托管账户，将租金返还给创建者

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{close_account, CloseAccount, Mint, TokenAccount, TokenInterface};
use crate::state::Escrow;
use crate::errors::EscrowError;

// =============================================================================
// Take 账户结构体
// =============================================================================
#[derive(Accounts)]
pub struct Take<'info> {
    // ------------------------------------------------------------------------
    // 签名者账户
    // ------------------------------------------------------------------------
    // 接受者：接受托管交易的用户
    #[account(mut)]
    pub taker: Signer<'info>,

    // 创建者：原始创建托管交易的用户
    // - mut: 可变（会接收代币 B 和托管账户的租金）
    // - SystemAccount: 普通的系统账户（不需要签名）
    #[account(mut)]
    pub maker: SystemAccount<'info>,

    // ------------------------------------------------------------------------
    // 托管账户（PDA）
    // ------------------------------------------------------------------------
    // 托管账户：包含交易状态的 PDA
    //
    // 约束说明：
    // - mut: 可变（会被关闭）
    // - close = maker: 关闭后，账户的 lamports 会返还给 maker
    // - seeds: 验证 PDA 是否正确派生
    //   - 使用 escrow.seed（从账户数据中读取）作为种子
    // - bump = escrow.bump: 验证 bump 种子是否匹配
    // - has_one: 验证托管账户中的字段是否与提供的账户匹配
    //   - has_one = maker: 验证 escrow.maker == maker.key()
    //   - has_one = mint_a: 验证 escrow.mint_a == mint_a.key()
    //   - has_one = mint_b: 验证 escrow.mint_b == mint_b.key()
    //   - 如果不匹配，返回指定的错误
    #[account(
    mut,
    close = maker,
    seeds = [b"escrow".as_ref(), maker.key().as_ref(),escrow.seed.to_le_bytes().as_ref()],
    bump = escrow.bump,
    has_one = maker @ EscrowError::InvalidMaker,
    has_one = mint_a @ EscrowError::InvalidMintA,
    has_one = mint_b @ EscrowError::InvalidMintB,
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    // ------------------------------------------------------------------------
    // 代币账户（Token Accounts）
    // ------------------------------------------------------------------------

    // 代币 A 的 Mint 账户：金库中存储的代币类型
    // - Box: 将账户分配到堆上，减少栈空间使用
    pub mint_a: Box<InterfaceAccount<'info,Mint>>,

    // 代币 B 的 Mint 账户：接受者需要发送的代币类型
    pub mint_b: Box<InterfaceAccount<'info,Mint>>,

    // 金库账户：存储代币 A 的关联代币账户
    // - mut: 可变（代币会被转出，账户会被关闭）
    // - associated_token::authority = escrow: 金库由 escrow PDA 控制
    #[account(
    mut,
    associated_token::mint = mint_a,
    associated_token::authority = escrow,
    associated_token::token_program = token_program,
    )]
    pub vault: Box<InterfaceAccount<'info,TokenAccount>>,

    // 接受者的代币 A ATA（可能不存在）
    // - init_if_needed: 如果账户不存在则创建，存在则跳过
    // - payer = taker: 由 taker 支付创建费用
    #[account(
      init_if_needed,
      payer = taker,
      associated_token::mint = mint_a,
      associated_token::authority = taker,
      associated_token::token_program = token_program
    )]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    // 接受者的代币 B ATA（用于发送代币给创建者）
    #[account(
      init_if_needed,
    payer = taker,
    associated_token::mint = mint_b,
    associated_token::authority = taker,
    associated_token::token_program = token_program,
    )]
    pub taker_ata_b: Box<InterfaceAccount<'info,TokenAccount>>,

    // 创建者的代币 B ATA（用于接收代币）
    // 可能不存在，需要 init_if_needed
    #[account(
    init_if_needed,
    payer = taker,
    associated_token::mint = mint_b,
    associated_token::authority = maker,
    associated_token::token_program = token_program,
    )]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    // ------------------------------------------------------------------------
    // 程序账户
    // ------------------------------------------------------------------------
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

// =============================================================================
// Take 指令的方法实现
// =============================================================================
impl<'info> Take<'info> {
    // ------------------------------------------------------------------------
    // transfer_to_maker: 向创建者发送代币 B
    // ------------------------------------------------------------------------
    // 将指定数量的代币 B 从接受者转移到创建者
    //
    // 数量来源：
    //   self.escrow.receive - 从托管账户中读取创建者期望的数量
    //
    // CPI 说明：
    //   调用 Token Program 的 transfer_checked 指令
    //   由 taker 签名授权转账
    fn transfer_to_maker(&mut self) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked{
                    from: self.taker_ata_b.to_account_info(),    // 从：接受者的代币 B ATA
                    to: self.maker_ata_b.to_account_info(),     // 到：创建者的代币 B ATA
                    mint: self.mint_b.to_account_info(),        // mint：代币 B 的 mint 账户
                    authority: self.taker.to_account_info(),    // 权限：接受者必须签名
                },
            ),
            self.escrow.receive,   // 转账数量（托管账户中记录的期望数量）
            self.mint_b.decimals   // 代币 B 的精度
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // withdraw_and_close_vault: 从金库提取代币并关闭金库账户
    // ------------------------------------------------------------------------
    // 1. 将金库中的所有代币 A 转移给接受者
    // 2. 关闭金库账户，将剩余 lamports 返还给创建者
    //
    // PDA 签名说明：
    //   金库的 authority 是 escrow PDA，没有私钥
    //   需要使用 CpiContext::new_with_signer 提供 PDA 签名
    //   signer_seeds 包含派生 PDA 使用的所有种子 + bump
    //
    // 关闭账户说明：
    //   close_account 指令会将：
    //   1. 账户中的代币全部转出
    //   2. 账户的 lamports 余额转给 destination
    //   3. 将账户数据清零，账户可以被重新分配
    fn withdraw_and_close_vault(&mut self) -> Result<()> {
        // 构造 PDA 签名种子
        // 必须与派生 escrow PDA 时使用的种子顺序完全一致
        let signer_seeds: [&[&[u8]]; 1] = [&[
            b"escrow",                                        // 固定前缀
            self.maker.to_account_info().key.as_ref(),       // 创建者公钥
            &self.escrow.seed.to_le_bytes()[..],             // 随机种子
            &[self.escrow.bump],                             // bump 种子
        ]];

        // 步骤 1: 将金库中的代币 A 转移给接受者
        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked{
                    from:self.vault.to_account_info(),          // 从：金库账户
                    to:self.taker_ata_a.to_account_info(),     // 到：接受者的代币 A ATA
                    mint:self.mint_a.to_account_info(),        // mint：代币 A 的 mint 账户
                    authority:self.escrow.to_account_info(),   // 权限：escrow PDA（需要签名）
                },
                &signer_seeds,    // PDA 签名（通过种子提供）
            ),
            self.vault.amount,      // 转账数量：金库中的全部代币
            self.mint_a.decimals,   // 代币 A 的精度
        )?;

        // 步骤 2: 关闭金库账户
        // 将金库账户的 lamports 返还给创建者
        close_account(CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            CloseAccount{
                account:self.vault.to_account_info(),
                authority: self.escrow.to_account_info(),
                destination: self.maker.to_account_info()
            },
            &signer_seeds,
        ))?;
        Ok(())
    }
}

// =============================================================================
// Take 指令的 Handler 函数
// =============================================================================
// 这是 Take 指令的主处理函数
//
// 执行顺序很重要：
// 1. 先转账代币 B（确保接受者有足够的代币）
// 2. 再提取代币 A（如果转账失败，不会释放金库）
//
// 为什么先转账后提取？
// - 如果先提取，但接受者没有足够的代币 B，交易会回滚
// - 但这样可能会让攻击者反复尝试，消耗创建者的资源
// - 先转账可以确保接受者确实有足够的代币
pub fn handler(ctx: Context<Take>) -> Result<()> {
    // 步骤 1: 接受者向创建者发送代币 B
    ctx.accounts.transfer_to_maker()?;

    // 步骤 2: 从金库提取代币 A 给接受者，并关闭金库
    ctx.accounts.withdraw_and_close_vault()?;

    Ok(())
}
