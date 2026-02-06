use anchor_lang::prelude::*; 
use anchor_spl::{ 
    associated_token::AssociatedToken, // 关联代币程序类型。
    token_interface::{ // Token 接口模块，兼容 SPL Token / Token-2022。
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface, // CPI 方法与账户类型。
        TransferChecked, // TransferChecked CPI 账户结构。
    }, 
}; 
// 本 crate 内部导入。 
use crate::{ 
    errors::EscrowError, // 自定义错误定义。
    state::{Escrow}, // Escrow 状态与 PDA 种子。
}; // crate 导入结束。
// take 指令的账户上下文。
#[derive(Accounts)] // 派生账户校验逻辑。
pub struct Take<'info> { 
  #[account(mut)] // taker 需可变，用于支付租金并签名。
  pub taker: Signer<'info>, // taker 签名者账户。
  #[account(mut)] // maker 可能接收 lamports。
  pub maker: SystemAccount<'info>, // maker 系统账户。
  #[account( 
      mut, 
      close = maker, // 关闭 escrow 并把租金返还给 maker。
      seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()], // PDA 种子。
      bump = escrow.bump, // 校验 PDA bump。
      has_one = maker @ EscrowError::InvalidMaker, // 校验 maker 与 escrow 一致。
      has_one = mint_a @ EscrowError::InvalidMintA, // 校验 mint A 与 escrow 一致。
      has_one = mint_b @ EscrowError::InvalidMintB, // 校验 mint B 与 escrow 一致。
  )] 
  pub escrow: Box<Account<'info, Escrow>>, // escrow PDA 账户。
  // Token 账户与 mint。 
  pub mint_a: Box<InterfaceAccount<'info, Mint>>, // mint A 账户。
  pub mint_b: Box<InterfaceAccount<'info, Mint>>, // mint B 账户。
  #[account( // 金库 ATA（escrow 持有的 Token A）。
      mut, // 金库将被扣款并关闭。
      associated_token::mint = mint_a, // 金库 mint 必须为 mint A。
      associated_token::authority = escrow, // 金库权限为 escrow PDA。
      associated_token::token_program = token_program // 金库使用指定 token_program。
  )] // 金库约束结束。
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>, // 金库存放 Token A。
  #[account( // taker 的 ATA（mint A）。
      init_if_needed, // 若不存在则创建。
      payer = taker, // 由 taker 支付创建费用。
      associated_token::mint = mint_a, // ATA mint 为 mint A。
      associated_token::authority = taker, // ATA 权限为 taker。
      associated_token::token_program = token_program // ATA 使用指定 token_program。
  )] // taker ATA A 约束结束。
  pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>, // taker 的 Token A 账户。
  #[account( // taker 的 ATA（mint B）。
      mut, // taker ATA B 将被扣款。
      associated_token::mint = mint_b, // ATA mint 为 mint B。
      associated_token::authority = taker, // ATA 权限为 taker。
      associated_token::token_program = token_program // ATA 使用指定 token_program。
  )] // taker ATA B 约束结束。
  pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>, // taker 的 Token B 账户。
  #[account( // maker 的 ATA（mint B）。
      init_if_needed, // 若不存在则创建。
      payer = taker, // 由 taker 支付创建费用。
      associated_token::mint = mint_b, // ATA mint 为 mint B。
      associated_token::authority = maker, // ATA 权限为 maker。
      associated_token::token_program = token_program // ATA 使用指定 token_program。
  )] // maker ATA B 约束结束。
  pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>, // maker 的 Token B 账户。
  // 程序账户。 
  pub associated_token_program: Program<'info, AssociatedToken>, // 关联代币程序。
  pub token_program: Interface<'info, TokenInterface>, // Token 程序接口。
  pub system_program: Program<'info, System>, // 系统程序。
} // Take 账户结构体结束。
// Take 辅助方法实现。 
impl<'info> Take<'info> { // Take 的 impl 开始。
    fn transfer_to_maker(&mut self) -> Result<()> { // 从 taker 转 Token B 给 maker。
        transfer_checked( // CPI 调用 Token 程序（带 decimals 校验）。
            CpiContext::new( // 构造 CPI 上下文。
                self.token_program.to_account_info(), // Token 程序账户。
                TransferChecked { // TransferChecked CPI 账户集合。
                    from: self.taker_ata_b.to_account_info(), // 转出：taker ATA B。
                    to: self.maker_ata_b.to_account_info(), // 转入：maker ATA B。
                    mint: self.mint_b.to_account_info(), // mint B 账户。
                    authority: self.taker.to_account_info(), // 授权者：taker 签名者。
                }, // TransferChecked 账户结束。
            ), // CPI 上下文结束。
            self.escrow.receive, // 转账数量（escrow 中约定的 receive）。
            self.mint_b.decimals, // mint B 精度。
        )?; // 传播 CPI 错误。
        Ok(()) 
    } // transfer_to_maker 结束。
    // 从金库取出 Token A 并关闭金库。 
    fn withdraw_and_close_vault(&mut self) -> Result<()> { // 处理金库提取与关闭。
        // 构造金库 PDA 的签名种子。 
        let signer_seeds: [&[&[u8]]; 1] = [&[ // 构造 signer seeds 数组。
            b"escrow", // 种子前缀。
            self.maker.to_account_info().key.as_ref(), // maker 公钥种子。
            &self.escrow.seed.to_le_bytes()[..], // escrow seed 的字节序。
            &[self.escrow.bump], // PDA bump。
        ]]; 
        // 转移 Token A（金库 -> taker）。 // 转账说明。
        transfer_checked( // CPI 调用 Token 程序并使用 PDA 签名。
            CpiContext::new_with_signer( // 构造带签名的 CPI 上下文。
                self.token_program.to_account_info(), // Token 程序账户。
                TransferChecked { // TransferChecked CPI 账户集合。
                    from: self.vault.to_account_info(), // 转出：金库 ATA。
                    to: self.taker_ata_a.to_account_info(), // 转入：taker ATA A。
                    mint: self.mint_a.to_account_info(), // mint A 账户。
                    authority: self.escrow.to_account_info(), // 授权者：escrow PDA。
                }, // TransferChecked 账户结束。
                &signer_seeds, // PDA 签名种子。
            ), // CPI 上下文结束。
            self.vault.amount, // 转出金库全部余额。
            self.mint_a.decimals, // mint A 精度。
        )?; // 传播 CPI 错误。
        // 关闭金库账户。 // 关闭说明。
        close_account(CpiContext::new_with_signer( // CPI 关闭金库。
            self.token_program.to_account_info(), // Token 程序账户。
            CloseAccount { // CloseAccount CPI 账户集合。
                account: self.vault.to_account_info(), // 要关闭的金库账户。
                authority: self.escrow.to_account_info(), // 授权者：escrow PDA。
                destination: self.maker.to_account_info(), // 关闭后 lamports 归 maker。
            }, // CloseAccount 账户结束。
            &signer_seeds, // PDA 签名种子。
        ))?; // 传播 CPI 错误。
        Ok(()) // 返回成功。
    } // withdraw_and_close_vault 结束。
} // Take 的 impl 结束。
// take 指令处理器。 
pub fn handler(ctx: Context<Take>) -> Result<()> { // take 入口逻辑。
    // 将 Token B 转给 maker。 
    ctx.accounts.transfer_to_maker()?; // 执行 Token B 转账。
    // 提取 Token A 并关闭金库。 
    ctx.accounts.withdraw_and_close_vault()?; // 转出 Token A 并关闭金库。
    Ok(()) // 返回成功。
} // take 处理器结束。
