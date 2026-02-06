// =============================================================================
// Pinocchio 托管程序 - 主入口文件
// =============================================================================
// 这是一个使用 Pinocchio 框架编写的 Solana 智能合约，实现无需信任的代币交换托管系统

// 导入 Pinocchio 核心组件：
// - AccountView: 用于读取和操作账户数据的视图
// - Address: 表示 Solana 地址（公钥/程序 ID）
// - entrypoint: 宏，用于定义程序的入口点
// - ProgramResult: 程序执行结果的类型别名（Result<(), ProgramError>）
use pinocchio::{AccountView, Address, entrypoint, ProgramResult};

// 导入错误类型，用于处理程序运行时的错误情况
use pinocchio::error::ProgramError;

// 声明程序的入口点函数
// Solana 运行时会调用这个函数来执行程序逻辑
entrypoint!(process_instruction);

// =============================================================================
// 模块声明与导出
// =============================================================================

// instructions 模块：包含所有指令处理器（Make, Take, Refund）
pub mod instructions;
pub use instructions::*;

// errors 模块：包含自定义错误类型
pub mod errors;
pub use errors::*;

// state 模块：包含托管账户的数据结构定义
pub mod state;
pub use state::*;

#[cfg(test)]
pub mod tests;

// =============================================================================
// 程序 ID（Program ID）
// =============================================================================
// 这是程序的唯一标识符（32 字节的公钥）
// 在部署时由 Solana 工具链生成，用于识别和调用此程序
// 所有客户端交易都需要指定这个程序 ID 才能调用此合约
// 以下字节代表的公钥是：22222222222222222222222222222222222222222222
pub const ID: Address = Address::new_from_array(
    [
        0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07,
        0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
        0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07,
        0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
    ]
);

// =============================================================================
// 程序入口点函数
// =============================================================================
// 这是 Solana 运行时调用的主函数，所有交易请求都会经过这里
//
// 参数说明：
// - _program_id: 当前程序的 ID（此处未使用，因为有常量 ID）
// - accounts: 交易中传入的所有账户列表（可变和只读账户）
// - instruction_data: 指令数据，包含操作类型和参数
//
// 返回值：
// - ProgramResult: 成功返回 Ok(())，失败返回 Err(ProgramError)
//
// 指令路由机制：
// 程序使用"判别器（Discriminator）"模式来路由不同的指令
// 每个指令都有一个唯一的字节（DISCRIMINATOR）作为标识
// Solana 运行时会将 instruction_data 的第一个字节与判别器匹配
// 来决定调用哪个指令处理器
fn process_instruction(
    _program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    // split_first() 将 instruction_data 分成第一个字节（判别器）和剩余数据
    // 使用模式匹配来路由到对应的指令处理器
    match instruction_data.split_first() {
        // Make 指令：创建托管交易
        // - 解析: 传入指令数据和账户列表
        // - Make::try_from: 验证账户并解析指令数据
        // - .process(): 执行业务逻辑
        Some((Make::DISCRIMINATOR, data)) => Make::try_from((data, accounts))?.process(),

        // Take 指令：接受托管交易
        // - 无额外数据，只需要账户列表
        Some((Take::DISCRIMINATOR, _)) => Take::try_from(accounts)?.process(),

        // Refund 指令：取消托管交易并退款
        // - 无额外数据，只需要账户列表
        Some((Refund::DISCRIMINATOR, _)) => Refund::try_from(accounts)?.process(),

        // 如果判别器不匹配任何已知指令，返回错误
        _ => Err(ProgramError::InvalidInstructionData)
    }
}