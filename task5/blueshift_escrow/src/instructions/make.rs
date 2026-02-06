// =============================================================================
// Make 指令 - Pinocchio 版本
// =============================================================================
// 本指令用于创建一个新的托管交易
// 创建者将代币 A 存入金库，并指定希望获得的代币 B 数量
//
// 与 Anchor 版本的对应关系见下方各部分注释

use pinocchio::{Address, AccountView, ProgramResult};
use pinocchio::cpi::Seed;
use pinocchio::error::ProgramError;
use pinocchio_token::instructions::Transfer;
use crate::{AccountCheck, SignerAccount, MintInterface, AssociatedTokenAccount, AssociatedTokenAccountCheck, ProgramAccount, Escrow, ProgramAccountInit, AssociatedTokenAccountInit};

// =============================================================================
// MakeAccounts 账户结构体
// =============================================================================
// 对应 Anchor 中的 Make<'info> 结构体
//
// Anchor 版本（make_anchor.rs:10-51）：
//   #[derive(Accounts)]
//   #[instruction(seed: u64)]
//   pub struct Make<'info> {
//       #[account(mut)]
//       pub maker: Signer<'info>,
//       #[account(init, payer = maker, space = ..., seeds = [...], bump)]
//       pub escrow: Account<'info, Escrow>,
//       pub mint_a: InterfaceAccount<'info, Mint>,
//       pub mint_b: InterfaceAccount<'info, Mint>,
//       pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
//       #[account(init, ...)]
//       pub vault: InterfaceAccount<'info, TokenAccount>,
//       pub system_program: Program<'info, System>,
//       pub token_program: Interface<'info, TokenInterface>,
//   }
//
// Pinocchio 版本差异：
// - 使用生命周期参数 'info 代替 Anchor 的 'info
// - 每个字段都是 &AccountView 引用（原始账户视图）
// - 需要手动验证账户（通过 TryFrom trait）
pub struct MakeAccounts<'info> {
    // 创建者账户
    // 对应 Anchor: #[account(mut)] pub maker: Signer<'info>
    pub maker: &'info AccountView,

    // 托管账户（PDA）
    // 对应 Anchor: #[account(init, payer = maker, space = ..., seeds = [...], bump)]
    //            pub escrow: Account<'info, Escrow>
    pub escrow: &'info AccountView,

    // 代币 A 的 Mint 账户
    // 对应 Anchor: pub mint_a: InterfaceAccount<'info, Mint>
    pub mint_a: &'info AccountView,

    // 代币 B 的 Mint 账户
    // 对应 Anchor: pub mint_b: InterfaceAccount<'info, Mint>
    pub mint_b: &'info AccountView,

    // 创建者的代币 A ATA
    // 对应 Anchor: #[account(mut, associated_token::mint = mint_a,
    //            associated_token::authority = maker, ...)]
    //            pub maker_ata_a: InterfaceAccount<'info, TokenAccount>
    pub maker_ata_a: &'info AccountView,

    // 金库账户（PDA）
    // 对应 Anchor: #[account(init, payer = maker,
    //            associated_token::mint = mint_a,
    //            associated_token::authority = escrow, ...)]
    //            pub vault: InterfaceAccount<'info, TokenAccount>
    pub vault: &'info AccountView,

    // 系统程序
    // 对应 Anchor: pub system_program: Program<'info, System>
    pub system_program: &'info AccountView,

    // 代币程序
    // 对应 Anchor: pub token_program: Interface<'info, TokenInterface>
    pub token_program: &'info AccountView,
}

// =============================================================================
// TryFrom 实现 - 账户解析与验证
// =============================================================================
// 对应 Anchor 的 #[account(...)] 约束验证
//
// Anchor 版本通过宏自动生成验证代码
// Pinocchio 版本需要手动编写验证逻辑
impl<'info> TryFrom<&'info [AccountView]> for MakeAccounts<'info> {
    type Error = ProgramError;

    // 从账户数组中解析和验证账户
    // 对应 Anchor 自动进行的账户验证
    fn try_from(accounts: &'info [AccountView]) -> Result<Self, Self::Error> {
        // 解构账户数组
        // 对应 Anchor 自动按字段名顺序解析账户
        let [maker, escrow, mint_a, mint_b, maker_ata_a, vault, system_program, token_program, _] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // =====================================================================
        // 账户验证
        // =====================================================================
        // 对应 Anchor 的各种 #[account(...)] 约束
        //
        // Anchor 自动验证：
        // - Signer 类型自动检查 is_signer()
        // - Account<T> 自动检查 owner 和数据长度
        // - InterfaceAccount<Mint> 自动检查 owner 和数据
        // - associated_token 约束自动计算并验证 PDA
        //
        // Pinocchio 手动验证：
        // - 调用各个类型的 check() 方法

        // 验证 maker 是签名者
        // 对应 Anchor: pub maker: Signer<'info>
        // Signer 类型自动验证账户已签名
        SignerAccount::check(maker)?;

        // 验证 mint_a 是有效的 Mint 账户
        // 对应 Anchor: pub mint_a: InterfaceAccount<'info, Mint>
        // InterfaceAccount 自动验证：
        // 1. owner 是 Token Program 或 Token-2022
        // 2. 账户数据长度正确
        MintInterface::check(mint_a)?;

        // 验证 mint_b 是有效的 Mint 账户
        // 对应 Anchor: pub mint_b: InterfaceAccount<'info, Mint>
        MintInterface::check(mint_b)?;

        // 验证 maker_ata_a 是正确的 ATA
        // 对应 Anchor: #[account(
        //     mut,
        //     associated_token::mint = mint_a,
        //     associated_token::authority = maker,
        //     associated_token::token_program = token_program
        // )]
        // pub maker_ata_a: InterfaceAccount<'info, TokenAccount>
        //
        // associated_token 约束自动：
        // 1. 验证账户是有效的 Token Account
        // 2. 计算 ATA 的 PDA 地址：[authority, token_program, mint]
        // 3. 验证计算出的地址与传入的账户地址匹配
        AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;

        // 注意：escrow 和 vault 的验证在 try_from 中跳过
        // 因为它们会在后续的 init 过程中创建

        // 返回验证通过的账户结构
        // 对应 Anchor 自动生成的账户结构实例
        Ok(Self {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ata_a,
            vault,
            system_program,
            token_program,
        })
    }
}

// =============================================================================
// MakeInstructionData 指令数据结构体
// =============================================================================
// 对应 Anchor 的 handler 函数参数
//
// Anchor 版本（make_anchor.rs:157）：
//   pub fn handler(
//       ctx: Context<Make>,
//       seed: u64,      // ← 这些参数由 Anchor 自动解析
//       receive: u64,
//       amount: u64,
//   ) -> Result<()> {
//
// Pinocchio 版本：
// - 指令数据是字节数组 &[u8]
// - 需要手动解析为结构体
pub struct MakeInstructionData {
    // PDA 派生种子
    // 对应 Anchor: #[instruction(seed: u64)] + handler 参数 seed
    pub seed: u64,

    // 希望获得的代币 B 数量
    // 对应 Anchor: handler 参数 receive
    pub receive: u64,

    // 实际存入的代币 A 数量
    // 对应 Anchor: handler 参数 amount
    pub amount: u64,
}

// =============================================================================
// TryFrom 实现 - 指令数据解析
// =============================================================================
// 对应 Anchor 自动解析指令参数
impl<'info> TryFrom<&'info [u8]> for MakeInstructionData {
    type Error = ProgramError;

    // 从字节数组解析指令数据
    // 对应 Anchor 自动将 instruction_data 解析为函数参数
    fn try_from(data: &'info [u8]) -> Result<Self, Self::Error> {
        // 验证数据长度：3 个 u64 = 24 字节
        // 对应 Anchor 自动验证参数类型
        if data.len() != size_of::<u64>() * 3 {
            return Err(ProgramError::InvalidInstructionData);
        }

        // 解析三个 u64 值（小端序）
        // 对应 Anchor 自动反序列化参数
        let seed = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let receive = u64::from_le_bytes(data[8..16].try_into().unwrap());
        let amount = u64::from_le_bytes(data[16..24].try_into().unwrap());

        // =====================================================================
        // 业务逻辑验证
        // =====================================================================
        // 对应 Anchor: require_gt!(receive, 0, EscrowError::InvalidAmount);
        //              require_gt!(amount, 0, EscrowError::InvalidAmount);
        //
        // Anchor 使用 require_gt! 宏进行验证
        // Pinocchio 手动编写验证逻辑

        // 验证数量必须大于 0
        if amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            seed,
            receive,
            amount,
        })
    }
}

// =============================================================================
// Make 指令主结构体
// =============================================================================
// 对应 Anchor 的 Context<Make> + 指令数据的组合
//
// Anchor 版本：
// - ctx.accounts: 包含所有账户
// - ctx.remaining_accounts: 剩余账户
// - ctx.bumps: PDA bump 值
// - handler 参数：seed, receive, amount
//
// Pinocchio 版本：
// - accounts: 账户结构
// - instruction_data: 解析后的指令数据
// - bump: PDA bump 种子
pub struct Make<'info> {
    pub accounts: MakeAccounts<'info>,
    pub instruction_data: MakeInstructionData,
    pub bump: u8,
}

// =============================================================================
// TryFrom 实现 - 指令完整解析
// =============================================================================
// 对应 Anchor 的 Context 解析 + 账户初始化
impl<'info> TryFrom<(&'info [u8], &'info [AccountView])> for Make<'info> {
    type Error = ProgramError;

    // 从指令数据和账户数组中解析完整的指令
    // 对应 Anchor 自动进行的：
    // 1. 账户验证（#[account] 宏）
    // 2. 参数解析（handler 参数）
    // 3. init 约束处理（创建账户）
    fn try_from((data, accounts): (&'info [u8], &'info [AccountView])) -> Result<Self, Self::Error> {
        // 步骤 1: 解析和验证账户
        // 对应 Anchor 的账户验证阶段
        let accounts = MakeAccounts::try_from(accounts)?;

        // 步骤 2: 解析指令数据
        // 对应 Anchor 的参数解析
        let instruction_data = MakeInstructionData::try_from(data)?;

        // =====================================================================
        // 账户初始化
        // =====================================================================
        // 对应 Anchor 的 init 约束
        //
        // Anchor 版本（make_anchor.rs:56-62）：
        //   #[account(
        //       init,                    // ← 创建新账户
        //       payer = maker,           // ← maker 支付费用
        //       space = Escrow::INIT_SPACE + 8,
        //       seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()],
        //       bump,                    // ← 自动计算并存储 bump
        //   )]
        //   pub escrow: Account<'info, Escrow>,
        //
        // Pinocchio 手动实现：
        // 1. 计算 PDA 和 bump
        // 2. 构造签名种子
        // 3. 调用 System Program 创建账户

        // 计算 PDA 地址和 bump
        // 对应 Anchor 的 seeds 和 bump 自动处理
        let (_, bump) = Address::find_program_address(
            &[
                b"escrow",                                      // 固定前缀
                accounts.maker.address().as_ref(),             // 创建者地址
                &instruction_data.seed.to_le_bytes(),          // 随机种子
            ],
            &crate::ID,  // 程序 ID
        );

        // 构造 PDA 签名种子
        // 对应 Anchor 自动生成的 signer_seeds
        // 需要绑定生命周期，因为 Seed 引用这些数据
        let seed_binding = instruction_data.seed.to_le_bytes();
        let bump_binding = [bump];
        let escrow_seeds = [
            Seed::from(b"escrow"),                           // 种子 1: "escrow"
            Seed::from(accounts.maker.address().as_ref().as_ref()),  // 种子 2: maker 地址
            Seed::from(&seed_binding),                       // 种子 3: seed 的字节数组
            Seed::from(&bump_binding),                       // 种子 4: bump
        ];

        // 创建托管账户
        // 对应 Anchor 的 init 约束自动调用 System Program
        // helpers.rs 中的 ProgramAccountInit::init 实现：
        // 1. 计算租金豁免所需的 lamports
        // 2. 创建 PDA 签名者
        // 3. 调用 CreateAccount 指令
        ProgramAccount::init::<Escrow>(
            accounts.maker,      // payer：对应 Anchor 的 payer = maker
            accounts.escrow,     // 要创建的账户
            &escrow_seeds,       // PDA 签名种子：对应 Anchor 的 seeds
            Escrow::LEN,         // 账户大小：对应 Anchor 的 space = ...
        )?;

        // =====================================================================
        // 金库账户初始化
        // =====================================================================
        // 对应 Anchor 的 vault init 约束
        //
        // Anchor 版本（make_anchor.rs:101-107）：
        //   #[account(
        //       init,                    // ← 创建新 ATA
        //       payer = maker,           // ← maker 支付费用
        //       associated_token::mint = mint_a,
        //       associated_token::authority = escrow,
        //       associated_token::token_program = token_program
        //   )]
        //   pub vault: InterfaceAccount<'info, TokenAccount>,
        //
        // Pinocchio 手动实现：
        // 调用 Associated Token Account Program 创建 ATA

        // 创建金库 ATA
        // 对应 Anchor 的 init 约束自动调用 ATA Program
        // helpers.rs 中的 AssociatedTokenAccount::init 实现：
        // 通过 CPI 调用 Associated Token Account Program
        AssociatedTokenAccount::init(
            accounts.vault,           // 要创建的金库账户
            accounts.mint_a,          // mint 账户
            accounts.maker,           // payer：对应 Anchor 的 payer = maker
            accounts.escrow,          // owner：对应 Anchor 的 authority = escrow
            accounts.system_program,  // System Program
            accounts.token_program,   // Token Program
        )?;

        // 返回完整的指令结构
        Ok(Self {
            accounts,
            instruction_data,
            bump,  // 保存 bump 用于后续签名
        })
    }
}

// =============================================================================
// Make 指令的业务逻辑实现
// =============================================================================
impl<'info> Make<'info> {
    // 指令判别器
    // 对应 Anchor 自动生成的指令判别器（8 字节哈希）
    // Pinocchio 使用单个字节，更高效
    pub const DISCRIMINATOR: &'info u8 = &0;

    // 处理函数：执行托管交易创建的业务逻辑
    // 对应 Anchor 的 handler 函数（make_anchor.rs:209-230）
    //
    // Anchor 版本：
    //   pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
    //       // 验证参数（已在 try_from 中完成）
    //       ctx.accounts.populate_escrow(seed, receive, ctx.bumps.escrow)?;
    //       ctx.accounts.deposit_token(amount)?;
    //       Ok(())
    //   }
    //
    // Pinocchio 版本：
    // - 直接在 process 方法中实现业务逻辑
    // - 不需要单独的 populate_escrow 和 deposit_token 方法
    pub fn process(&mut self) -> ProgramResult {
        // =====================================================================
        // 步骤 1: 初始化托管账户数据
        // =====================================================================
        // 对应 Anchor: ctx.accounts.populate_escrow(seed, receive, ctx.bumps.escrow)
        //              (make_anchor.rs:143-153)
        //
        // Anchor 版本使用 set_inner 方法一次性设置所有字段
        // Pinocchio 版本直接操作字节数组

        // 获取托管账户的可变借用
        let mut data = self.accounts.escrow.try_borrow_mut()?;

        // 将字节数组解析为 Escrow 结构体
        // unsafe transmute 将字节指针转换为结构体指针
        let escrow = Escrow::load_mut(data.as_mut())?;

        // 设置托管账户的所有字段
        // 对应 Anchor 的 set_inner 方法（make_anchor.rs:143-152）
        escrow.set_inner(
            self.instruction_data.seed,                   // seed：PDA 派生种子
            self.accounts.maker.address().clone(),        // maker：创建者地址
            self.accounts.mint_a.address().clone(),       // mint_a：代币 A mint
            self.accounts.mint_b.address().clone(),       // mint_b：代币 B mint
            self.instruction_data.receive.clone(),        // receive：期望数量
            [self.bump],                                 // bump：PDA bump 种子
        );

        // =====================================================================
        // 步骤 2: 存入代币到金库
        // =====================================================================
        // 对应 Anchor: ctx.accounts.deposit_token(amount)
        //              (make_anchor.rs:174-189)
        //
        // Anchor 版本使用 CPI 调用 transfer_checked：
        //   transfer_checked(
        //       CpiContext::new(...),
        //       amount,
        //       self.mint_a.decimals  // ← Anchor 自动传递 decimals
        //   )
        //
        // Pinocchio 版本使用 Transfer 指令（不需要 decimals）
        // 因为 Transfer 指令使用 Token Program 的基本转账功能

        // 转账代币 A 从创建者 ATA 到金库
        // 对应 Anchor 的 transfer_checked CPI 调用
        Transfer {
            from: self.accounts.maker_ata_a,   // 从：创建者的代币 A ATA
            to: self.accounts.vault,           // 到：金库账户
            authority: self.accounts.maker,    // 权限：创建者必须签名
            amount: self.instruction_data.amount  // 转账数量
        }.invoke()?;  // 调用 Token Program 执行转账

        Ok(())
    }
}
