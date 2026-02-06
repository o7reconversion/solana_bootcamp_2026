use anchor_lang::prelude::*; // 引入 Anchor 预导入内容（宏与常用类型）。
// 托管程序的错误定义。 // 章节标题行，避免空行。
#[error_code] // 标记该枚举为 Anchor 错误集合。
pub enum EscrowError { // 托管相关错误枚举开始。
    #[msg("Invalid amount")] // 当数量非法时的错误消息。
    InvalidAmount, // 数量必须大于 0 或满足约束。
    #[msg("Invalid maker")] // 当 maker 地址不匹配时的错误消息。
    InvalidMaker, // maker 账户与 escrow 数据不一致。
    #[msg("Invalid mint a")] // 当 mint A 不匹配时的错误消息。
    InvalidMintA, // mint A 与 escrow 配置不一致。
    #[msg("Invalid mint b")] // 当 mint B 不匹配时的错误消息。
    InvalidMintB, // mint B 与 escrow 配置不一致。
} 
