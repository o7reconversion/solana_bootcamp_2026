// =============================================================================
// Refund 指令 - Anchor 版本
// =============================================================================
// 本指令用于取消托管交易并退还代币
// 只有创建者可以调用此指令，将存入的代币 A 取回
//
// 执行流程：
// 1. 验证调用者是托管交易的创建者
// 2. 从金库中将代币 A 转移回创建者
// 3. 关闭金库账户，将剩余 lamports 返还给创建者
// 4. 关闭托管账户，将租金返还给创建者
//
// 使用场景：
// - 创建者改变主意，不再想进行交易
// - 长时间内没有人接受托管交易
// - 创建者需要紧急取回代币

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{close_account, CloseAccount, Mint, TokenAccount, TokenInterface};
use crate::errors::EscrowError;
use crate::state::Escrow;

// =============================================================================
// Refund 账户结构体
// =============================================================================
#[derive(Accounts)]
pub struct Refund<'info> {
    // ------------------------------------------------------------------------
    // 签名者账户
    // ------------------------------------------------------------------------
    // 创建者：必须是原始创建托管交易的用户
    // - mut: 可变（会接收代币 A 和托管账户的租金）
    // - Signer: 必须签名（验证身份）
    #[account(mut)]
    pub maker: Signer<'info>,

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
    // - has_one = maker: 验证 escrow.maker == maker.key()
    //   确保只有创建者才能退款
    // - has_one = mint_a: 验证 escrow.mint_a == mint_a.key()
    //   确保使用正确的代币类型
    #[account(
    mut,
    close = maker,
    seeds = [b"escrow".as_ref(), maker.key().as_ref(),escrow.seed.to_le_bytes().as_ref()],
    bump = escrow.bump,
    has_one = maker @ EscrowError::InvalidMaker,
    has_one = mint_a @ EscrowError::InvalidMintA,
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    // ------------------------------------------------------------------------
    // 代币账户（Token Accounts）
    // ------------------------------------------------------------------------

    // 代币 A 的 Mint 账户：金库中存储的代币类型
    #[account(
        mint::token_program = token_program
    )]
    pub mint_a: InterfaceAccount<'info, Mint>,

    // 金库账户：存储代币 A 的关联代币账户
    // - mut: 可变（代币会被转出，账户会被关闭）
    // - associated_token::authority = escrow: 金库由 escrow PDA 控制
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    // 创建者的代币 A ATA（用于接收代币）
    // - init_if_needed: 如果账户不存在则创建，存在则跳过
    // - payer = maker: 由 maker 支付创建费用
    #[account(
        init_if_needed,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    // ------------------------------------------------------------------------
    // 程序账户
    // ------------------------------------------------------------------------
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

// =============================================================================
// Refund 指令的方法实现
// =============================================================================
impl<'info> Refund<'info> {
    // ------------------------------------------------------------------------
    // withdraw_and_close_vault: 从金库提取代币并关闭金库账户
    // ------------------------------------------------------------------------
    // 1. 将金库中的所有代币 A 转移回创建者
    // 2. 关闭金库账户，将剩余 lamports 返还给创建者
    //
    // PDA 签名说明：
    //   金库的 authority 是 escrow PDA，没有私钥
    //   需要使用 CpiContext::new_with_signer 提供 PDA 签名
    //   signer_seeds 包含派生 PDA 使用的所有种子 + bump
    //
    // 安全性：
    //   - has_one 约束确保只有创建者能调用此指令
    //   - PDA 签名确保只有本程序能控制金库
    //   - 关闭托管账户（close = maker）确保交易完成后不能重复退款
    fn withdraw_and_close_vault(&mut self) -> Result<()> {

        // 构造 PDA 签名种子
        // 必须与派生 escrow PDA 时使用的种子顺序完全一致
        let signer_seeds: [&[&[u8]]; 1] = [&[
            b"escrow",                                        // 固定前缀
            self.maker.to_account_info().key.as_ref(),       // 创建者公钥
            &self.escrow.seed.to_le_bytes()[..],             // 随机种子
            &[self.escrow.bump],                             // bump 种子
        ]];

        // 将代币 vault 转移到 maker_ata_a
        // Transfer Token A (Vault -> Maker)
        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.vault.to_account_info(),          // 从：金库账户
                    to: self.maker_ata_a.to_account_info(),     // 到：创建者的代币 A ATA
                    mint: self.mint_a.to_account_info(),        // mint：代币 A 的 mint 账户
                    authority: self.escrow.to_account_info(),   // 权限：escrow PDA（需要签名）
                },
                &signer_seeds,    // PDA 签名（通过种子提供）
            ),
            self.vault.amount,      // 转账数量：金库中的全部代币
            self.mint_a.decimals,   // 代币 A 的精度
        )?;

        // Close the Vault
        // 关闭金库账户
        close_account(CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            CloseAccount {
                account: self.vault.to_account_info(),        // 要关闭的账户：金库
                authority: self.escrow.to_account_info(),     // 权限：escrow PDA（金库的 owner）
                destination: self.maker.to_account_info(),    // 接收 lamports 的账户：创建者
            },
            &signer_seeds,    // PDA 签名
        )?;
        Ok(())
    }
}

// =============================================================================
// Refund 指令的 Handler 函数
// =============================================================================
// 这是 Refund 指令的主处理函数
//
// 执行步骤：
// 1. Anchor 验证所有账户约束（包括 has_one 验证）
// 2. 从金库提取代币并关闭金库账户
// 3. Anchor 自动关闭托管账户（close = maker 约束）
//
// 为什么托管账户在最后自动关闭？
// - close = maker 约束在指令执行完毕后生效
// - 这样确保只有在所有操作成功后才关闭账户
// - 如果前面的操作失败，托管账户不会被关闭，可以重试
pub fn handler(ctx: Context<Refund>) -> Result<()> {
    // 从金库提取代币 A 给创建者，并关闭金库
    ctx.accounts.withdraw_and_close_vault()?;

    // 托管账户会在函数返回后自动关闭（由 Anchor 的 close = maker 约束处理）
    // 关闭后，租金会返还给创建者

    Ok(())
}
