use anchor_lang::prelude::*; 
use anchor_spl::{ // 引入 Anchor SPL 辅助与 Token 接口。
    associated_token::AssociatedToken, // 关联代币程序类型。
    token_interface::{ // Token 接口模块，兼容 SPL Token / Token-2022。
        transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked, // CPI 方法与账户类型。
    }, // token_interface 导入结束。
}; // anchor_spl 导入结束。

use crate::{ // 引入当前 crate 的内容。
    errors::EscrowError, // 自定义错误定义。
    state::{Escrow}, // Escrow 状态与 PDA 种子常量。
}; // crate 导入结束。
// make 指令的账户上下文。 
#[derive(Accounts)] // 派生账户校验逻辑。
#[instruction(seed: u64)] // make 使用 seed 作为 PDA 种子参数。
pub struct Make<'info> { // Make 账户结构体开始。
    #[account(mut)] // maker 需要可变，用于支付租金与签名转账。
    pub maker: Signer<'info>, // 创建托管的 maker 签名者。
    #[account( // escrow PDA 账户约束。
        init, // 初始化 escrow 账户。
        payer = maker, // 由 maker 支付租金。
        space = Escrow::INIT_SPACE + Escrow::DISCRIMINATOR.len(), // 分配所需空间与鉴别器。
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()], // escrow PDA 种子。
        bump, // 由 Anchor 计算并记录 bump。
    )] // escrow 账户约束结束。
    pub escrow: Account<'info, Escrow>, // 保存交易条款的 escrow PDA 账户。
    // Token 账户与 mint。 
    #[account( // mint A 约束。
        mint::token_program = token_program // mint A 必须属于当前 token_program。
    )] // mint A 约束结束。
    pub mint_a: InterfaceAccount<'info, Mint>, // Token A 的 mint。
    #[account( // mint B 约束。
        mint::token_program = token_program // mint B 必须属于当前 token_program。
    )] // mint B 约束结束。
    pub mint_b: InterfaceAccount<'info, Mint>, // Token B 的 mint。
    #[account( // maker 的 ATA（mint A）。
        mut, // 该账户将被扣款。
        associated_token::mint = mint_a, // ATA 必须是 mint A。
        associated_token::authority = maker, // ATA 权限为 maker。
        associated_token::token_program = token_program // ATA 使用指定 token_program。
    )] // maker ATA 约束结束。
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>, // maker 的 Token A 账户。
    #[account( // escrow 的金库 ATA。
        init, // 创建金库 ATA。
        payer = maker, // maker 支付金库租金。
        associated_token::mint = mint_a, // 金库 ATA 对应 mint A。
        associated_token::authority = escrow, // 金库权限为 escrow PDA。
        associated_token::token_program = token_program // 金库使用指定 token_program。
    )] // 金库 ATA 约束结束。
    pub vault: InterfaceAccount<'info, TokenAccount>, // 存放 Token A 的金库账户。
    // 程序账户。
    pub associated_token_program: Program<'info, AssociatedToken>, // 关联代币程序。
    pub token_program: Interface<'info, TokenInterface>, // Token 程序接口。
    pub system_program: Program<'info, System>, // 系统程序。
} 
// Make 辅助方法实现。
impl<'info> Make<'info> { // Make 的 impl 开始。
    pub fn populate_escrow(&mut self, seed: u64, receive: u64, bump: u8) -> Result<()> { // 填充 escrow 字段。
        self.escrow.seed = seed; // 保存 seed 用于后续 PDA 推导。
        self.escrow.maker = self.maker.key(); // 保存 maker 公钥。
        self.escrow.mint_a = self.mint_a.key(); // 保存 mint A 公钥。
        self.escrow.mint_b = self.mint_b.key(); // 保存 mint B 公钥。
        self.escrow.receive = receive; // 保存期望接收的 Token B 数量。
        self.escrow.bump = bump; // 保存 PDA bump。
        Ok(()) 
    } // populate_escrow 结束。
    // 将 maker 的 Token A 存入金库。
    pub fn deposit_tokens(&mut self, amount: u64) -> Result<()> { // 转账 Token A 到金库。
        transfer_checked( // CPI 调用 Token 程序（带 decimals 校验）。
            CpiContext::new( // 构造 CPI 上下文。
                self.token_program.to_account_info(), // Token 程序账户。
                TransferChecked { // TransferChecked CPI 账户集合。
                    from: self.maker_ata_a.to_account_info(), // 转出：maker ATA。
                    mint: self.mint_a.to_account_info(), // mint A 账户。
                    to: self.vault.to_account_info(), // 转入：金库 ATA。
                    authority: self.maker.to_account_info(), // 授权者：maker 签名者。
                }, 
            ), 
            amount, 
            self.mint_a.decimals, 
        )?; 
        Ok(()) 
    } 
} 
// make 指令处理器。 
pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> { // make 入口逻辑。
    // 校验数量参数。 // 校验说明。
    require_gt!(receive, 0, EscrowError::InvalidAmount); // receive 必须大于 0。
    require_gt!(amount, 0, EscrowError::InvalidAmount); // deposit 必须大于 0。
    // 写入 Escrow 数据。 // 状态初始化说明。
    ctx.accounts.populate_escrow(seed, receive, ctx.bumps.escrow)?; // 持久化 escrow 字段。
    // 存入 Token。 // 转账说明。
    ctx.accounts.deposit_tokens(amount)?; // 将 maker 的 Token A 存入金库。
    Ok(()) // 返回成功。
} 
