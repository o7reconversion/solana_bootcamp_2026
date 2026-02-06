// =============================================================================
// 错误模块 - 自定义错误类型定义
// =============================================================================
// 本模块定义了托管程序可能返回的所有自定义错误
// 这些错误会作为 ProgramError::Custom() 返回给客户端

use pinocchio::error::ProgramError;
use core::fmt;

// =============================================================================
// EscrowError 自定义错误枚举
// =============================================================================
// 定义程序中所有可能的错误情况
//
// 每个错误都有一个明确的数值（从 0 开始递增），这个数值会被编码到
// ProgramError::Custom(error as u32) 中返回给客户端
//
// 客户端可以通过这个错误码来识别具体的错误类型，并进行相应的处理
//
// derive 属性说明：
// - Clone: 允许错误被复制
// - Debug: 允许使用 {:?} 格式化输出（用于调试）
// - Eq: 允许错误之间进行相等比较
// - PartialEq: 允许错误进行部分相等比较
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowError {
    /// 账户余额低于租金豁免阈值
    /// Solana 要求账户必须持有足够的 lamports 以免被删除
    /// 如果余额不足，账户可能被垃圾回收
    NotRentExempt = 0,

    /// 账户未签名
    /// 某些操作需要特定的账户签名（如创建者、接受者）
    /// 如果应该签名的账户没有签名，返回此错误
    NotSigner = 1,

    /// 非法的账户所有者
    /// Solana 账户由特定程序拥有（通过 owner 字段指定）
    /// 如果账户的所有者不是预期的程序，返回此错误
    /// 例如：Token Account 的 owner 应该是 Token Program
    InvalidOwner = 2,

    /// 非法的账户数据
    /// 账户数据的长度或格式不符合预期
    /// 例如：期望的账户数据长度与实际不匹配
    InvalidAccountData = 3,

    /// 非法的地址
    /// 提供的地址不符合预期要求
    /// 例如：PDA 派生失败、地址不匹配等
    InvalidAddress = 4,
}

// =============================================================================
// From<EscrowError> for ProgramError Trait 实现
// =============================================================================
// 这个实现允许将 EscrowError 自动转换为 ProgramError
//
// Rust 的 From trait 提供了一种类型转换机制
// 实现了 From<EscrowError> for ProgramError 后，可以使用：
// - Err(EscrowError::NotSigner)? 自动转换为 Err(ProgramError::Custom(1))
// - ProgramError::from(EscrowError::NotSigner) 显式转换
//
// 在程序中使用 '?' 操作符时，如果返回的是 EscrowError，
// 会自动调用这个 from 方法转换为 ProgramError
//
// error as u32:
// 将枚举转换为它的数值表示（discriminant）
// 例如：EscrowError::NotRentExempt => 0
//       EscrowError::NotSigner => 1
impl From<EscrowError> for ProgramError {
    fn from(error: EscrowError) -> Self {
        ProgramError::Custom(error as u32)
    }
}

// =============================================================================
// fmt::Display Trait 实现
// =============================================================================
// 这个实现允许将 EscrowError 格式化为可读的字符串
//
// 用途：
// - 使用 println!("{}", error) 打印错误信息
// - 在日志中记录人类可读的错误描述
// - 客户端显示友好的错误消息
//
// 返回值：
// - fmt::Result: 实际上是 Result<(), fmt::Error>
//   格式化成功返回 Ok(())，失败返回 Err(fmt::Error)
//
// 注意：
// 这些字符串消息主要用于调试和日志
// 客户端通常通过错误码（数值）来识别错误类型
impl fmt::Display for EscrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EscrowError::NotRentExempt => write!(f, "Lamport balance below rent-exempt threshold"),
            EscrowError::NotSigner => write!(f, "没有签名"),
            EscrowError::InvalidOwner => write!(f, "非法的所有者"),
            EscrowError::InvalidAccountData => write!(f, "非法的账户数据"),
            EscrowError::InvalidAddress => write!(f, "非法的地址"),
        }
    }
}