// =============================================================================
// 辅助工具模块 - Pinocchio 账户验证和初始化
// =============================================================================
// 本模块提供了 Pinocchio 框架中手动实现的账户验证功能
// 这些功能对应 Anchor 框架中的各种账户约束宏
//
// Anchor vs Pinocchio：
// - Anchor 使用 #[account(...)] 宏自动生成验证代码
// - Pinocchio 需要手动编写验证逻辑，但更灵活、性能更好
//
// 本模块通过 Trait 和零大小类型（ZST）实现类型安全的账户验证

use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::Sysvar;
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::instructions::CreateAccount;
use crate::errors::EscrowError;

// =============================================================================
// AccountCheck Trait - 基础账户验证
// =============================================================================
// 定义账户验证的统一接口
//
// 对应 Anchor 的约束：
// - 所有账户验证的基础
// - 可以组合多个约束使用
pub trait AccountCheck {
    // 验证账户是否符合要求
    // 如果不符合，返回相应的错误
    fn check(account: &AccountView) -> Result<(), ProgramError>;
}

// =============================================================================
// AssociatedTokenAccountCheck Trait - ATA 验证
// =============================================================================
// 验证关联代币账户（ATA）的接口
//
// 对应 Anchor 的约束：
// - associated_token::authority = xxx
// - associated_token::mint = xxx
// - associated_token::token_program = xxx
//
// ATA 是通过 PDA 派生的特殊代币账户，地址由以下决定：
// - owner 的地址
// - token_program 的地址
// - mint 的地址
pub trait AssociatedTokenAccountCheck{
    // 验证账户是否是指定 owner、mint 和 token_program 的 ATA
    fn check(
        account: &AccountView,
        authority: &AccountView,    // 对应 Anchor 中的 authority 约束
        mint: &AccountView,          // 对应 Anchor 中的 mint 约束
        token_program: &AccountView, // 对应 Anchor 中的 token_program 约束
    ) -> Result<(), ProgramError>;
}

// =============================================================================
// AssociatedTokenAccountInit Trait - ATA 创建
// =============================================================================
// 创建关联代币账户的接口
//
// 对应 Anchor 的约束：
// - init: 创建新账户
// - init_if_needed: 如果账户不存在则创建
pub trait AssociatedTokenAccountInit{
    // 创建新的 ATA
    // 对应 Anchor 的 init 约束
    fn init(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,      // 对应 Anchor 的 payer = xxx
        owner: &AccountView,      // 对应 Anchor 的 authority = xxx
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult;

    // 如果账户不存在则创建，存在则跳过
    // 对应 Anchor 的 init_if_needed 约束
    fn init_if_needed(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        owner: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult;
}

// =============================================================================
// SignerAccount - 签名者账户验证
// =============================================================================
// 对应 Anchor 的约束：#[account(signer)]
//
// Anchor 版本：
//   #[account(signer)]
//   pub maker: Signer<'info>,
//
// Pinocchio 版本：
//   使用 SignerAccount 类型 + AccountCheck trait 验证
//
// 功能：
// - 验证账户是否签名（is_signer()）
// - 必须由私钥持有者签名（或 PDA 签名）
//
// 使用场景：
// - 需要授权操作的账户（如创建者、接受者）
// - 需要支付费用的账户
pub struct SignerAccount;

impl AccountCheck for SignerAccount {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        // is_signer() 检查账户是否在交易的签名者列表中
        // 对应 Anchor 的 Signer 类型自动进行的验证
        if !account.is_signer() {
            return Err(EscrowError::NotSigner.into());
        }
        Ok(())
    }
}

// =============================================================================
// SystemAccount - 系统账户验证
// =============================================================================
// 对应 Anchor 的约束：SystemAccount<'info>
//
// Anchor 版本：
//   pub maker: SystemAccount<'info>,
//
// 功能：
// - 验证账户由 System Program 拥有
// - 不要求签名
//
// 使用场景：
// - 接收资金的普通账户（如 Take 指令中的 maker）
// - 只读的系统账户
pub struct SystemAccount;

impl AccountCheck for SystemAccount {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        // owned_by() 检查账户的 owner 是否为指定程序
        // System Program 的 ID 是固定的
        if !account.owned_by(&pinocchio_system::ID) {
            return Err(EscrowError::InvalidOwner.into());
        }

        Ok(())
    }
}

// =============================================================================
// Token-2022 Program 常量
// =============================================================================
// Token-2022 是新版 Token Program，与原版兼容但增加了扩展功能

// Token-2022 Program ID
// TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
pub const TOKEN_2022_PROGRAM_ID: Address = Address::new_from_array(
    [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
]);

// Token-2022 账户中判别器的偏移量
// Token-2022 在账户数据的第 165 字节存储判别器
const TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET: usize = 165;

// Token-2022 Mint 账户的判别器值
pub const TOKEN_2022_MINT_DISCRIMINATOR: u8 = 0x01;

// Token-2022 Token Account 的判别器值
pub const TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR: u8 = 0x02;

// =============================================================================
// MintInterface - Mint 账户验证（支持 Token Program 和 Token-2022）
// =============================================================================
// 对应 Anchor 的约束：InterfaceAccount<'info, Mint>
//
// Anchor 版本：
//   pub mint_a: InterfaceAccount<'info, Mint>,
//
// 功能：
// - 验证账户由 Token Program 或 Token-2022 Program 拥有
// - 验证账户数据长度正确
// - 支持 Token Program 的两个版本
//
// 为什么需要 Interface？
// - Token Program 有两个版本（旧版和 Token-2022）
// - InterfaceAccount 可以同时支持两个版本
// - 提供更好的兼容性
pub struct MintInterface;

impl AccountCheck for MintInterface {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        // 检查是否由 Token-2022 Program 拥有
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            // 如果不是 Token-2022，检查是否是旧版 Token Program
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(EscrowError::InvalidOwner.into());
            } else {
                // 旧版 Token Program 的 Mint 账户长度验证
                if account.data_len().ne(&pinocchio_token::state::Mint::LEN) {
                    return Err(EscrowError::InvalidAccountData.into());
                }
            }
        } else {
            // Token-2022 的 Mint 账户验证
            let data = account.try_borrow()?;

            // Token-2022 账户可能更长（因为有扩展）
            // 如果长度等于旧版长度，说明没有扩展，直接通过
            if data.len().ne(&pinocchio_token::state::Mint::LEN) {
                // 如果长度小于判别器偏移量，数据无效
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(EscrowError::InvalidAccountData.into());
                }
                // 检查判别器是否为 Mint 类型（0x01）
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR) {
                    return Err(EscrowError::InvalidAccountData.into());
                }
            }
        }

        Ok(())
    }
}

// =============================================================================
// TokenAccountInterface - Token Account 验证
// =============================================================================
// 对应 Anchor 的约束：InterfaceAccount<'info, TokenAccount>
//
// Anchor 版本：
//   pub vault: InterfaceAccount<'info, TokenAccount>,
//
// 功能：
// - 验证账户由 Token Program 或 Token-2022 Program 拥有
// - 验证账户数据长度和判别器
// - 支持两个版本的 Token Program
pub struct TokenAccountInterface;

impl AccountCheck for TokenAccountInterface {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        // 检查是否由 Token-2022 Program 拥有
        if !account.owned_by(&TOKEN_2022_PROGRAM_ID) {
            // 如果不是 Token-2022，检查是否是旧版 Token Program
            if !account.owned_by(&pinocchio_token::ID) {
                return Err(EscrowError::InvalidOwner.into());
            } else {
                // 旧版 Token Account 长度验证
                if account.data_len().ne(&pinocchio_token::state::TokenAccount::LEN) {
                    return Err(EscrowError::InvalidAccountData.into());
                }
            }
        } else {
            // Token-2022 Token Account 验证
            let data = account.try_borrow()?;

            if data.len().ne(&pinocchio_token::state::TokenAccount::LEN) {
                // 检查长度是否足够包含判别器
                if data.len().le(&TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET) {
                    return Err(EscrowError::InvalidAccountData.into());
                }
                // 检查判别器是否为 Token Account 类型（0x02）
                if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                    .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
                {
                    return Err(EscrowError::InvalidAccountData.into());
                }
            }
        }

        Ok(())
    }
}

// =============================================================================
// AssociatedTokenAccount - 关联代币账户验证
// =============================================================================
// 对应 Anchor 的约束：
// - associated_token::authority = xxx
// - associated_token::mint = xxx
// - associated_token::token_program = xxx
//
// Anchor 版本：
//   #[account(
//       associated_token::authority = maker,
//       associated_token::mint = mint_a,
//       associated_token::token_program = token_program
//   )]
//   pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
//
// 验证逻辑：
// 1. 先验证是否是有效的 Token Account
// 2. 计算 ATA 的 PDA 地址
// 3. 验证计算出的地址与传入的账户地址是否匹配
pub struct AssociatedTokenAccount;

impl AssociatedTokenAccountCheck for AssociatedTokenAccount {
    fn check(
        account: &AccountView,
        authority: &AccountView,
        mint: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError> {
        // 先验证账户是否是有效的 Token Account
        TokenAccountInterface::check(account)?;

        // 计算 ATA 的 PDA 地址
        // ATA 的派生种子：[authority, token_program, mint]
        let (pda, _bump) = Address::find_program_address(
            &[
                authority.address().as_ref(),    // 所有者地址
                token_program.address().as_ref(),  // Token Program 地址
                mint.address().as_ref(),           // Mint 地址
            ],
            &pinocchio_associated_token_account::ID,  // ATA Program ID
        );

        // 将计算出的 PDA 地址转换为 Pinocchio 的 Address 类型
        let pda_address = Address::new_from_array(pda.to_bytes());

        // 验证计算出的 PDA 地址是否与传入的账户地址匹配
        // 这确保传入的账户确实是正确的 ATA
        if pda_address.ne(account.address()) {
            return Err(EscrowError::InvalidAddress.into());
        }

        Ok(())
    }
}

// =============================================================================
// AssociatedTokenAccount 创建实现
// =============================================================================
impl AssociatedTokenAccountInit for AssociatedTokenAccount {
    // 创建新的 ATA
    // 对应 Anchor 的 init 约束
    //
    // 过程：
    // 1. 通过 CPI 调用 Associated Token Account Program
    // 2. 创建 ATA 账户
    // 3. 设置 authority 和 mint
    fn init(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        owner: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        Create {
            funding_account: payer,       // 支付创建费用的账户
            account,                      // 要创建的 ATA 账户
            wallet: owner,                // ATA 的所有者
            mint,                         // 关联的 mint 账户
            system_program,               // System Program
            token_program,                // Token Program
        }.invoke()
    }

    // 如果账户不存在则创建
    // 对应 Anchor 的 init_if_needed 约束
    //
    // 逻辑：
    // 1. 先尝试验证账户是否存在且正确
    // 2. 如果验证通过，说明账户已存在，直接返回
    // 3. 如果验证失败，说明账户不存在，创建新账户
    fn init_if_needed(
        account: &AccountView,
        mint: &AccountView,
        payer: &AccountView,
        owner: &AccountView,
        system_program: &AccountView,
        token_program: &AccountView,
    ) -> ProgramResult {
        match Self::check(account, owner, mint, token_program) {
            Ok(_) => Ok(()),  // 账户已存在且正确，跳过创建
            Err(_) => Self::init(account, mint, payer, owner, system_program, token_program),  // 创建账户
        }
    }
}

// =============================================================================
// ProgramAccount - 程序自定义账户验证
// =============================================================================
// 对应 Anchor 的约束：Account<'info, Escrow>
//
// Anchor 版本：
//   #[account(
//       mut,
//       seeds = [...],
//       bump = escrow.bump,
//   )]
//   pub escrow: Account<'info, Escrow>,
//
// 功能：
// - 验证账户由本程序拥有（owner == program_id）
// - 验证账户数据长度正确
//
// 注意：
// - PDA 验证（seeds、bump）需要在指令中单独进行
pub struct ProgramAccount;

impl AccountCheck for ProgramAccount {
    fn check(account: &AccountView) -> Result<(), ProgramError> {
        // 验证账户由本程序拥有
        // 对应 Anchor 的 Account<T> 自动进行的 owner 检查
        if !account.owned_by(&crate::ID) {
            return Err(EscrowError::InvalidOwner.into());
        }

        // 验证账户数据长度是否匹配 Escrow 结构体
        if account.data_len().ne(&crate::state::Escrow::LEN) {
            return Err(EscrowError::InvalidAccountData.into());
        }

        Ok(())
    }
}

// =============================================================================
// ProgramAccountInit Trait - 程序账户初始化
// =============================================================================
// 对应 Anchor 的约束：
// - init: 创建新账户
// - payer = xxx: 指定支付者
// - space = xxx: 指定账户大小
// - seeds = [...]: PDA 种子
// - bump: 自动计算或验证 bump
//
// Anchor 版本：
//   #[account(
//       init,
//       payer = maker,
//       space = Escrow::INIT_SPACE + 8,
//       seeds = [...],
//       bump,
//   )]
//   pub escrow: Account<'info, Escrow>,
pub trait ProgramAccountInit {
    // 创建程序拥有的 PDA 账户
    fn init<'a, T: Sized>(
        payer: &AccountView,      // 支付者（对应 payer = xxx）
        account: &AccountView,    // 要创建的账户
        seeds: &[Seed<'a>],       // PDA 种子（对应 seeds = [...]）
        space: usize,             // 账户大小（对应 space = xxx）
    ) -> ProgramResult;
}

impl ProgramAccountInit for ProgramAccount {
    fn init<'a, T: Sized>(
        payer: &AccountView,
        account: &AccountView,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
        // 获取租金豁免所需的 lamports 数量
        // 对应 Anchor 自动进行的租金计算
        let lamports = Rent::get()?.try_minimum_balance(space)?;

        // 使用种子创建 PDA 签名者
        // 对应 Anchor 的 bump 自动处理
        let signer = [Signer::from(seeds)];

        // 创建账户并设置为本程序拥有
        // invoke_signed 使用 PDA 签名
        CreateAccount {
            from: payer,              // 从支付者账户扣除 lamports
            to: account,              // 要创建的账户
            lamports,                 // 转账的 lamports 数量
            space: space as u64,      // 账户数据空间大小
            owner: &crate::ID,        // 账户拥有者：本程序
        }
            .invoke_signed(&signer)?;  // 使用 PDA 签名调用

        Ok(())
    }
}

// =============================================================================
// AccountClose Trait - 关闭账户
// =============================================================================
// 对应 Anchor 的约束：close = xxx
//
// Anchor 版本：
//   #[account(
//       mut,
//       close = maker,
//   )]
//   pub escrow: Account<'info, Escrow>,
//
// 功能：
// - 将账户的 lamports 转给指定账户
// - 将账户数据清零
// - 关闭账户（账户可以被重新分配）
//
// 注意：
// - Anchor 在指令执行完毕后自动处理 close 约束
// - Pinocchio 需要手动调用 close 方法
pub trait AccountClose {
    fn close(account: &AccountView, destination: &AccountView) -> ProgramResult;
}

impl AccountClose for ProgramAccount {
    fn close(account: &AccountView, destination: &AccountView) -> ProgramResult {
        {
            // 将账户数据的第一个字节设置为 0xff
            // 这是 Solana 的惯例，表示账户已关闭
            let mut data = account.try_borrow_mut()?;
            data[0] = 0xff;
        }

        // 将账户的 lamports 转给目标账户
        // 对应 Anchor 的 close = destination 约束
        destination.set_lamports(destination.lamports()+account.lamports());

        // 将账户大小缩减到 1 字节（只剩下 0xff 标记）
        account.resize(1)?;

        // 关闭账户
        // 此时账户的 lamports 已被转移，数据被清零
        account.close()
    }
}
