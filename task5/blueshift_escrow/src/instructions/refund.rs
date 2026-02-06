// =============================================================================
// Refund 指令 - Pinocchio 版本
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
// 与 Anchor 版本的对应关系见下方各部分注释

use pinocchio::{AccountView, ProgramResult};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio_token::instructions::{CloseAccount, Transfer};
use solana_address::Address;
use crate::{AccountCheck, AccountClose, AssociatedTokenAccount, AssociatedTokenAccountInit, Escrow, MintInterface, ProgramAccount, SignerAccount};

// =============================================================================
// RefundAccount 账户结构体
// =============================================================================
// 对应 Anchor 中的 Refund<'info> 结构体
//
// Anchor 版本（refund_anchor.rs:9-48）：
//   #[derive(Accounts)]
//   pub struct Refund<'info> {
//       #[account(mut)] pub maker: Signer<'info>,
//       #[account(mut, close = maker, seeds = [...], bump = escrow.bump,
//                has_one = maker, has_one = mint_a)]
//       pub escrow: Box<Account<'info, Escrow>>,
//       #[account(mint::token_program = token_program)]
//       pub mint_a: InterfaceAccount<'info, Mint>,
//       #[account(mut, associated_token::mint = mint_a,
//                associated_token::authority = escrow, ...)]
//       pub vault: InterfaceAccount<'info, TokenAccount>,
//       #[account(init_if_needed, payer = maker, ...)]
//       pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
//       pub associated_token_program: Program<'info, AssociatedToken>,
//       pub token_program: Interface<'info, TokenInterface>,
//       pub system_program: Program<'info, System>,
//   }
//
// Pinocchio 版本差异：
// - 使用生命周期参数 'info
// - 每个字段都是 &AccountView 引用
// - 账户数量更少（7个 vs Anchor 的 8个）
//   - 不需要 associated_token_program（Pinocchio 自动处理）
pub struct RefundAccount<'info> {
    // 创建者账户（必须签名）
    // 对应 Anchor: #[account(mut)] pub maker: Signer<'info>
    //
    // 安全性：
    // - 必须签名验证身份
    // - 只有创建者才能退款
    pub maker: &'info AccountView,

    // 托管账户（PDA，将被关闭）
    // 对应 Anchor: #[account(mut, close = maker, seeds = [...],
    //            bump = escrow.bump, has_one = maker @ EscrowError::InvalidMaker,
    //            has_one = mint_a @ EscrowError::InvalidMintA)]
    //            pub escrow: Box<Account<'info, Escrow>>
    //
    // has_one 约束说明：
    // - has_one = maker: 验证 escrow.maker == maker.key()
    //   确保只有创建者能退款
    // - has_one = mint_a: 验证 escrow.mint_a == mint_a.key()
    //   确保使用正确的代币类型
    pub escrow: &'info AccountView,

    // 代币 A 的 Mint 账户
    // 对应 Anchor: #[account(mint::token_program = token_program)]
    //            pub mint_a: InterfaceAccount<'info, Mint>
    pub mint_a: &'info AccountView,

    // 金库账户（将被关闭）
    // 对应 Anchor: #[account(mut, associated_token::mint = mint_a,
    //            associated_token::authority = escrow,
    //            associated_token::token_program = token_program)]
    //            pub vault: InterfaceAccount<'info, TokenAccount>
    //
    // 金库由 escrow PDA 控制，只有本程序能转出代币
    pub vault: &'info AccountView,

    // 创建者的代币 A ATA（可能不存在）
    // 对应 Anchor: #[account(init_if_needed, payer = maker,
    //            associated_token::mint = mint_a,
    //            associated_token::authority = maker, ...)]
    //            pub maker_ata_a: InterfaceAccount<'info, TokenAccount>
    pub maker_ata_a: &'info AccountView,

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
impl<'info> TryFrom<&'info [AccountView]> for RefundAccount<'info> {
    type Error = ProgramError;

    // 从账户数组中解析和验证账户
    // 对应 Anchor 自动进行的账户验证
    fn try_from(accounts: &'info [AccountView]) -> Result<Self, Self::Error> {
        // 解构账户数组
        // 对应 Anchor 自动按字段名顺序解析账户
        let [maker, escrow, mint_a, vault, maker_ata_a, system_program, token_program, _] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // =====================================================================
        // 账户验证
        // =====================================================================
        // 对应 Anchor 的各种 #[account(...)] 约束

        // 验证 maker 是签名者
        // 对应 Anchor: pub maker: Signer<'info>
        // Signer 类型自动验证账户已签名
        SignerAccount::check(maker)?;

        // 验证 escrow 是本程序拥有的账户
        // 对应 Anchor: pub escrow: Box<Account<'info, Escrow>>
        // Account<T> 自动验证 owner 和数据长度
        ProgramAccount::check(escrow)?;

        // 验证 mint_a 是有效的 Mint 账户
        // 对应 Anchor: pub mint_a: InterfaceAccount<'info, Mint>
        MintInterface::check(mint_a)?;

        // 跳过 ATA 验证
        // 原因：vault 和 maker_ata_a 的验证会在 CPI 调用中自动进行
        // Token Program 会验证账户的所有者和权限
        //
        // 对应 Anchor 中的 associated_token 约束验证
        // Anchor 在账户验证阶段检查，Pinocchio 延迟到 CPI 阶段

        // 返回验证通过的账户结构
        Ok(Self {
            maker,
            escrow,
            mint_a,
            vault,
            maker_ata_a,
            system_program,
            token_program,
        })
    }
}

// =============================================================================
// Refund 指令主结构体
// =============================================================================
// 对应 Anchor 的 Context<Refund>
pub struct Refund<'info> {
    pub accounts: RefundAccount<'info>,
}

// =============================================================================
// TryFrom 实现 - 指令完整解析与账户初始化
// =============================================================================
// 对应 Anchor 的 Context 解析 + init_if_needed 约束处理
impl<'info> TryFrom<&'info [AccountView]> for crate::Refund<'info> {
    type Error = ProgramError;

    // 从账户数组中解析完整的指令
    // 对应 Anchor 自动进行的：
    // 1. 账户验证（#[account] 宏）
    // 2. init_if_needed 约束处理（如果账户不存在则创建）
    fn try_from(accounts: &'info [AccountView]) -> Result<Self, Self::Error> {
        // 步骤 1: 解析和验证账户
        // 对应 Anchor 的账户验证阶段
        let accounts = RefundAccount::try_from(accounts)?;

        // =====================================================================
        // 条件账户初始化
        // =====================================================================
        // 对应 Anchor 的 init_if_needed 约束
        //
        // Anchor 版本（refund_anchor.rs:85-94）：
        //   #[account(
        //       init_if_needed,           // ← 如果账户不存在则创建
        //       payer = maker,            // ← maker 支付创建费用
        //       associated_token::mint = mint_a,
        //       associated_token::authority = maker,
        //       associated_token::token_program = token_program
        //   )]
        //   pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
        //
        // Pinocchio 手动实现：
        // 1. 先尝试验证账户是否存在
        // 2. 如果不存在，则创建新账户

        // 创建创建者的代币 A ATA（如果不存在）
        // 对应 Anchor: pub maker_ata_a 的 init_if_needed 约束
        // helpers.rs 中的 init_if_needed 实现：
        // - 先尝试验证账户（check）
        // - 如果验证失败，说明账户不存在，调用 init 创建
        AssociatedTokenAccount::init_if_needed(
            accounts.maker_ata_a,     // 要创建/验证的账户
            accounts.mint_a,          // mint 账户
            accounts.maker,           // payer：对应 Anchor 的 payer = maker
            accounts.maker,           // owner：对应 Anchor 的 authority = maker
            accounts.system_program,  // System Program
            accounts.token_program,   // Token Program
        )?;

        // 返回完整的指令结构
        Ok(Self {
            accounts,
        })
    }
}

// =============================================================================
// Refund 指令的业务逻辑实现
// =============================================================================
impl<'info> Refund<'info> {
    // 指令判别器
    // 对应 Anchor 自动生成的指令判别器（8 字节哈希）
    // Pinocchio 使用单个字节，更高效
    pub const DISCRIMINATOR: &'info u8 = &2;

    // 处理函数：执行退款业务逻辑
    // 对应 Anchor 的 handler 函数（refund_anchor.rs:181-189）
    //
    // Anchor 版本：
    //   pub fn handler(ctx: Context<Refund>) -> Result<()> {
    //       ctx.accounts.withdraw_and_close_vault()?;  // ← 提取代币并关闭金库
    //       // 托管账户会在函数返回后自动关闭（close = maker 约束）
    //       Ok(())
    //   }
    //
    // Pinocchio 版本：
    // - 直接在 process 方法中实现业务逻辑
    // - 手动管理借用生命周期
    // - 手动关闭托管账户
    pub fn process(&mut self) -> ProgramResult {
        // =====================================================================
        // 读取托管账户数据
        // =====================================================================
        // 对应 Anchor 的 ctx.accounts.escrow 自动反序列化
        //
        // Anchor 版本：通过 ctx.accounts.escrow 直接访问字段
        // Pinocchio 版本：手动借用和解析字节数组

        // 使用代码块来限制借用生命周期
        // 确保借用在步骤 2 开始前释放
        let (seed, bump) = {
            // 借用托管账户数据（只读）
            let data = self.accounts.escrow.try_borrow()?;

            // 将字节数组解析为 Escrow 结构体
            // unsafe transmute 将字节指针转换为结构体指针
            let escrow = Escrow::load(&data)?;

            // =================================================================
            // PDA 验证（额外安全检查）
            // =================================================================
            // 对应 Anchor 的 seeds 约束验证
            //
            // Anchor 版本（refund_anchor.rs:54-60）：
            //   seeds = [b"escrow".as_ref(), maker.key().as_ref(),
            //           escrow.seed.to_le_bytes().as_ref()],
            //   bump = escrow.bump,
            //
            // Anchor 自动验证：
            // 1. 计算预期的 PDA 地址
            // 2. 验证传入的账户地址是否匹配
            //
            // Pinocchio 手动验证：
            // 1. 从账户数据中读取种子和 bump
            // 2. 重新计算 PDA 地址
            // 3. 验证计算出的地址与传入的地址是否匹配

            // 重新计算 PDA 地址以验证账户有效性
            // 使用托管账户中存储的种子和 bump
            let escrow_key = Address::create_program_address(
                &[
                    b"escrow",                                    // 固定前缀
                    self.accounts.maker.address().as_ref(),     // 创建者地址
                    &escrow.seed.to_le_bytes(),                  // 从账户中读取的 seed
                    &escrow.bump,                                // 从账户中读取的 bump
                ],
                &crate::ID  // 程序 ID
            )?;

            // 验证计算出的 PDA 地址是否与传入的账户地址匹配
            // 这确保：
            // 1. 账户确实是使用正确的种子派生的
            // 2. 账户数据未被篡改
            // 3. 只有创建者（通过 has_one 隐式验证）能退款
            if &escrow_key != self.accounts.escrow.address() {
                return Err(ProgramError::InvalidAccountOwner);
            }

            // 提取需要的字段
            // 注意：不需要 mint_b 和 receive 字段
            (escrow.seed, escrow.bump)
        }; // ← data 在这里自动释放，借用结束

        // =====================================================================
        // 构造 PDA 签名种子
        // =====================================================================
        // 对应 Anchor 自动生成的 signer_seeds
        //
        // Anchor 版本（refund_anchor.rs:128-133）：
        //   let signer_seeds: [&[&[u8]]; 1] = [&[
        //       b"escrow",
        //       self.maker.to_account_info().key.as_ref(),
        //       &self.escrow.seed.to_le_bytes()[..],
        //       &[self.escrow.bump],
        //   ]];
        //
        // Pinocchio 需要：
        // 1. 绑定生命周期（因为 Seed 引用这些数据）
        // 2. 将数据转换为 Seed 类型

        let seed_binding = seed.to_le_bytes();
        let bump_binding = bump;
        let escrow_seeds = [
            Seed::from(b"escrow"),                           // 种子 1: "escrow"
            Seed::from(self.accounts.maker.address().as_ref()),  // 种子 2: maker 地址
            Seed::from(&seed_binding),                       // 种子 3: seed 的字节数组
            Seed::from(&bump_binding),                       // 种子 4: bump
        ];

        // 创建 PDA 签名者
        // 对应 Anchor 的 &signer_seeds 参数
        let signer = Signer::from(&escrow_seeds);

        // =====================================================================
        // 读取金库中的代币数量
        // =====================================================================
        // 从金库账户中读取代币余额
        //
        // Token Account 的数据结构（offset 64-72）：
        // - amount: u64 (8 字节)，表示代币数量
        //
        // 使用代码块来限制借用生命周期
        let amount = {
            // 借用金库账户数据
            let vault_data = self.accounts.vault.try_borrow()?;

            // 读取 amount 字段（偏移量 64，长度 8）
            // Token Account 结构体的第 9 个字段是 amount
            u64::from_le_bytes(vault_data[64..72].try_into().unwrap())
        }; // ← vault_data 在这里自动释放

        // =====================================================================
        // 业务逻辑执行
        // =====================================================================
        // 对应 Anchor: ctx.accounts.withdraw_and_close_vault()
        //              （refund_anchor.rs:124-164）

        // =====================================================================
        // 步骤 1: 从金库转移代币 A 回创建者
        // =====================================================================
        // 对应 Anchor: transfer_checked 调用（refund_anchor.rs:137-150）
        //
        // Anchor 版本使用 transfer_checked：
        //   transfer_checked(
        //       CpiContext::new_with_signer(...),
        //       self.vault.amount,
        //       self.mint_a.decimals  // ← Anchor 自动传递 decimals
        //   )
        //
        // Pinocchio 版本使用 Transfer 指令（不需要 decimals）

        // 转账代币 A 从金库回创建者的 ATA
        // 将创建者存入的代币全部退还
        Transfer {
            from: self.accounts.vault,        // 从：金库账户
            to: self.accounts.maker_ata_a,    // 到：创建者的代币 A ATA
            authority: self.accounts.escrow,  // 权限：escrow PDA（需要签名）
            amount,                           // 转账数量：金库中的全部代币
        }.invoke_signed(&[signer.clone()])?;  // ← 使用 PDA 签名调用

        // invoke_signed 说明：
        // - 金库的 authority 是 escrow PDA，没有私钥
        // - 需要使用 invoke_signed 提供 PDA 签名
        // - signer 包含派生 PDA 的所有种子

        // =====================================================================
        // 步骤 2: 关闭金库账户
        // =====================================================================
        // 对应 Anchor: close_account 调用（refund_anchor.rs:154-162）
        //
        // Anchor 版本：
        //   close_account(CpiContext::new_with_signer(...))
        //
        // Pinocchio 版本：
        //   CloseAccount { ... }.invoke_signed(&[signer])

        // 关闭金库账户
        // 将金库账户的 lamports 返还给创建者
        CloseAccount {
            account: self.accounts.vault,       // 要关闭的账户：金库
            destination: self.accounts.maker,   // 接收 lamports 的账户：创建者
            authority: self.accounts.escrow,    // 权限：escrow PDA（金库的 owner）
        }.invoke_signed(&[signer.clone()])?;  // ← 使用 PDA 签名调用

        // close_account 说明：
        // 1. 验证账户余额为 0（代币已全部转出）
        // 2. 将账户的 lamports 转给 destination
        // 3. 将账户数据清零，账户可以被重新分配

        // =====================================================================
        // 步骤 3: 关闭托管账户
        // =====================================================================
        // 对应 Anchor: close = maker 约束（refund_anchor.rs:55）
        //
        // Anchor 版本：
        //   #[account(mut, close = maker, ...)]
        //   pub escrow: Box<Account<'info, Escrow>>,
        //
        // Anchor 在指令执行完毕后自动处理 close 约束：
        // 1. 将账户的 lamports 转给 maker
        // 2. 将账户数据清零
        //
        // Pinocchio 版本：
        // 手动调用 ProgramAccount::close()

        // 关闭托管账户
        // 将托管账户的租金（lamports）返还给创建者
        ProgramAccount::close(
            self.accounts.escrow,     // 要关闭的账户：托管账户
            self.accounts.maker       // 接收 lamports 的账户：创建者
        )?;

        // close 方法说明（helpers.rs:507-527）：
        // 1. 将账户数据的第一个字节设置为 0xff（关闭标记）
        // 2. 将账户的 lamports 转给 destination
        // 3. 将账户大小缩减到 1 字节
        // 4. 关闭账户

        // =====================================================================
        // 执行完成
        // =====================================================================
        // 所有必要操作已完成：
        // 1. ✅ 代币 A 从金库退还给创建者
        // 2. ✅ 金库账户已关闭，lamports 返还给创建者
        // 3. ✅ 托管账户已关闭，租金返还给创建者
        //
        // 托管交易已取消：
        // - 创建者取回了存入的代币
        // - 所有账户已关闭，租金已返还
        // - 该托管交易无法再被 Take 或再次 Refund

        Ok(())
    }
}
