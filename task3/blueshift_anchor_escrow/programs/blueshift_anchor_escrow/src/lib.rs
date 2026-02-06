use anchor_lang::prelude::*; 
// 模块声明。
mod errors; // 错误定义模块。
mod instructions; // 指令模块（make/take/refund）。
mod state; // 状态定义模块。
use instructions::*; // 使用重新导出的指令账户类型。
// 程序 ID 声明。 
declare_id!("22222222222222222222222222222222222222222222"); 
// 程序入口模块。 
#[program] // Anchor 程序宏，生成入口函数。
pub mod anchor_escrow { // 程序模块名。
    use super::*; // 将外层作用域内容引入当前模块。
    // 指令：make（鉴别器 = 0）。
    #[instruction(discriminator = 0)] // make 指令自定义鉴别器。
    pub fn make(ctx: Context<Make>, seed: u64, deposit: u64, receive: u64) -> Result<()> { // make 入口函数。
        instructions::make::handler(ctx, seed, deposit, receive) // 调用 make 处理器。
    } 
    // 指令：take（鉴别器 = 1）。 
    #[instruction(discriminator = 1)] 
    pub fn take(ctx: Context<Take>) -> Result<()> { // take 入口函数。
        instructions::take::handler(ctx) // 调用 take 处理器。
    } 
    // 指令：refund（鉴别器 = 2）。
    #[instruction(discriminator = 2)] // refund 指令自定义鉴别器。
    pub fn refund(ctx: Context<Refund>) -> Result<()> { // refund 入口函数。
        instructions::refund::handler(ctx) // 调用 refund 处理器。
    } 
} 
