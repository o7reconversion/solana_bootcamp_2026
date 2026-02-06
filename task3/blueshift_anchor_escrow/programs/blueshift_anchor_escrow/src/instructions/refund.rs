use anchor_lang::prelude::*; 
use anchor_spl::{ 
    associated_token::AssociatedToken, // 关联代币程序类型。
    token_interface::{ // Token 接口模块，兼容 SPL Token / Token-2022。
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface, // CPI 方法与账户类型。
        TransferChecked, // TransferChecked CPI 账户结构。
    }, 
}; 

use crate::{ 
    errors::EscrowError, // 自定义错误定义。
    state::{Escrow, ESCROW_SEED}, // Escrow 状态与 PDA 种子。
}; 
// refund 指令的账户上下文。 
#[derive(Accounts)] // 派生账户校验逻辑。
pub struct Refund<'info> { // Refund 账户结构体开始。
    #[account(mut)] // maker 需要可变并签名。
    pub maker: Signer<'info>, // maker 签名者账户。
    #[account( // escrow PDA 账户约束。
        mut, // escrow 将被关闭，需要可变。
        close = maker, // 关闭 escrow 并把租金返还给 maker。
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()], // PDA 种子。
        bump = escrow.bump, // 校验 PDA bump。
        has_one = maker @ EscrowError::InvalidMaker, // 校验 maker 与 escrow 一致。
        has_one = mint_a @ EscrowError::InvalidMintA // 校验 mint A 与 escrow 一致。
    )] 
    pub escrow: Account<'info, Escrow>, // escrow PDA 账户。
    #[account(mint::token_program = token_program)] // mint A 必须属于 token_program。
    pub mint_a: InterfaceAccount<'info, Mint>, // mint A 账户。
    #[account( // 金库 ATA（escrow 持有的 Token A）。
        mut, // 金库将被扣款并关闭。
        associated_token::mint = mint_a, // 金库 mint 必须为 mint A。
        associated_token::authority = escrow, // 金库权限为 escrow PDA。
        associated_token::token_program = token_program // 金库使用指定 token_program。
    )] 
    pub vault: InterfaceAccount<'info, TokenAccount>, // 金库存放 Token A。
    #[account( // maker 的 ATA（mint A）。
        init_if_needed, // 若不存在则创建。
        payer = maker, // 由 maker 支付创建费用。
        associated_token::mint = mint_a, // ATA mint 为 mint A。
        associated_token::authority = maker, // ATA 权限为 maker。
        associated_token::token_program = token_program // ATA 使用指定 token_program。
    )] // maker ATA 约束结束。
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>, // maker 的 Token A 账户。
    // 程序账户。 
    pub associated_token_program: Program<'info, AssociatedToken>, // 关联代币程序。
    pub token_program: Interface<'info, TokenInterface>, // Token 程序接口。
    pub system_program: Program<'info, System>, // 系统程序。
} 
// refund 指令处理器。 
pub fn handler(ctx: Context<Refund>) -> Result<()> { // refund 入口逻辑。
    let vault_amount = ctx.accounts.vault.amount; // 读取金库当前余额。
    let escrow = &ctx.accounts.escrow; // 绑定 escrow 引用用于种子。
    let seed_bytes = escrow.seed.to_le_bytes(); // 将 seed 转为小端字节。
    let signer_seeds: &[&[u8]] = &[ // 构造 PDA 签名种子切片。
        ESCROW_SEED, // 种子前缀。
        escrow.maker.as_ref(), // maker 公钥种子。
        seed_bytes.as_ref(), // seed 字节。
        &[escrow.bump], // PDA bump。
    ]; // 签名种子结束。
    let signer = &[signer_seeds]; // CPI 签名者种子包装。
    if vault_amount > 0 { // 仅在金库有余额时转账。
        transfer_checked( // CPI 调用 Token 程序并使用 PDA 签名。
            CpiContext::new_with_signer( // 构造带签名的 CPI 上下文。
                ctx.accounts.token_program.to_account_info(), // Token 程序账户。
                TransferChecked { // TransferChecked CPI 账户集合。
                    from: ctx.accounts.vault.to_account_info(), // 转出：金库 ATA。
                    mint: ctx.accounts.mint_a.to_account_info(), // mint A 账户。
                    to: ctx.accounts.maker_ata_a.to_account_info(), // 转入：maker ATA A。
                    authority: ctx.accounts.escrow.to_account_info(), // 授权者：escrow PDA。
                }, // TransferChecked 账户结束。
                signer, // PDA 签名种子。
            ), // CPI 上下文结束。
            vault_amount, // 转出全部余额。
            ctx.accounts.mint_a.decimals, // mint A 精度。
        )?; // 传播 CPI 错误。
    } 
    close_account(CpiContext::new_with_signer( // CPI 关闭金库账户。
        ctx.accounts.token_program.to_account_info(), // Token 程序账户。
        CloseAccount { // CloseAccount CPI 账户集合。
            account: ctx.accounts.vault.to_account_info(), // 要关闭的金库账户。
            destination: ctx.accounts.maker.to_account_info(), // 关闭后 lamports 归 maker。
            authority: ctx.accounts.escrow.to_account_info(), // 授权者：escrow PDA。
        }, // CloseAccount 账户结束。
        signer, // PDA 签名种子。
    ))?; // 传播 CPI 错误。
    Ok(()) // 返回成功。
} 
