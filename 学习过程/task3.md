# Task3 托管程序：从开始到成功（含完整代码）

这份记录包含完整代码与可复现步骤，覆盖 make / take / refund 三个指令与账户顺序要求，适配 Anchor 0.32.1 + anchor-spl token_interface。

## 0. 目标

- Escrow 状态账户使用鉴别器 1
- make 指令鉴别器 0
- take 指令鉴别器 1
- refund 指令鉴别器 2

## 1. 初始化与依赖

1) 创建项目并进入目录：
```bash
anchor init blueshift_anchor_escrow
cd blueshift_anchor_escrow
```

2) 添加依赖：
```bash
cargo add anchor-lang --features init-if-needed
cargo add anchor-spl
```

3) 更新 `programs/blueshift_anchor_escrow/Cargo.toml`：
```toml
idl-build = ["anchor-lang/idl-build", "anchor-spl/idl-build"]
```

4) 同步程序 ID：
- `programs/blueshift_anchor_escrow/src/lib.rs` 的 `declare_id!`
- `Anchor.toml` 的 `programs.localnet`

## 2. 目录结构

```
programs/blueshift_anchor_escrow/src
├── instructions
│   ├── make.rs
│   ├── take.rs
│   ├── refund.rs
│   └── mod.rs
├── errors.rs
├── state.rs
└── lib.rs
```

## 3. 完整代码（按文件）

### programs/blueshift_anchor_escrow/src/lib.rs
```rust
use anchor_lang::prelude::*;

mod errors;
mod instructions;
mod state;

use instructions::*;

declare_id!("22222222222222222222222222222222222222222222");

#[program]
pub mod anchor_escrow {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(ctx: Context<Make>, seed: u64, deposit: u64, receive: u64) -> Result<()> {
        instructions::make::handler(ctx, seed, deposit, receive)
    }

    #[instruction(discriminator = 1)]
    pub fn take(ctx: Context<Take>) -> Result<()> {
        instructions::take::handler(ctx)
    }

    #[instruction(discriminator = 2)]
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        instructions::refund::handler(ctx)
    }
}
```

### programs/blueshift_anchor_escrow/src/state.rs
```rust
use anchor_lang::prelude::*;

pub const ESCROW_SEED: &[u8] = b"escrow";

#[derive(InitSpace)]
#[account(discriminator = 1)]
pub struct Escrow {
    pub seed: u64,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub receive: u64,
    pub bump: u8,
}
```

### programs/blueshift_anchor_escrow/src/errors.rs
```rust
use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Invalid maker")]
    InvalidMaker,
    #[msg("Invalid mint a")]
    InvalidMintA,
    #[msg("Invalid mint b")]
    InvalidMintB,
}
```

### programs/blueshift_anchor_escrow/src/instructions/mod.rs
```rust
pub mod make;
pub mod take;
pub mod refund;

pub use make::*;
pub use take::*;
pub use refund::*;
```

### programs/blueshift_anchor_escrow/src/instructions/make.rs
```rust
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
    },
};

use crate::{
    errors::EscrowError,
    state::{Escrow, ESCROW_SEED},
};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Make<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    #[account(
        init,
        payer = maker,
        space = Escrow::INIT_SPACE + Escrow::DISCRIMINATOR.len(),
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,

    /// Token Accounts
    #[account(
        mint::token_program = token_program
    )]
    pub mint_a: InterfaceAccount<'info, Mint>,
    #[account(
        mint::token_program = token_program
    )]
    pub mint_b: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    /// Programs
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Make<'info> {
    pub fn populate_escrow(&mut self, seed: u64, receive: u64, bump: u8) -> Result<()> {
        self.escrow.seed = seed;
        self.escrow.maker = self.maker.key();
        self.escrow.mint_a = self.mint_a.key();
        self.escrow.mint_b = self.mint_b.key();
        self.escrow.receive = receive;
        self.escrow.bump = bump;
        Ok(())
    }

    pub fn deposit_tokens(&mut self, amount: u64) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.maker_ata_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    to: self.vault.to_account_info(),
                    authority: self.maker.to_account_info(),
                },
            ),
            amount,
            self.mint_a.decimals,
        )?;
        Ok(())
    }
}

pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
    // Validate the amount
    require_gt!(receive, 0, EscrowError::InvalidAmount);
    require_gt!(amount, 0, EscrowError::InvalidAmount);

    // Save the Escrow Data
    ctx.accounts.populate_escrow(seed, receive, ctx.bumps.escrow)?;

    // Deposit Tokens
    ctx.accounts.deposit_tokens(amount)?;

    Ok(())
}
```

### programs/blueshift_anchor_escrow/src/instructions/take.rs
```rust
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

use crate::{
    errors::EscrowError,
    state::{Escrow, ESCROW_SEED},
};

#[derive(Accounts)]
pub struct Take<'info> {
  #[account(mut)]
  pub taker: Signer<'info>,
  #[account(mut)]
  pub maker: SystemAccount<'info>,
  #[account(
      mut,
      close = maker,
      seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
      bump = escrow.bump,
      has_one = maker @ EscrowError::InvalidMaker,
      has_one = mint_a @ EscrowError::InvalidMintA,
      has_one = mint_b @ EscrowError::InvalidMintB,
  )]
  pub escrow: Box<Account<'info, Escrow>>,

  /// Token Accounts
  pub mint_a: Box<InterfaceAccount<'info, Mint>>,
  pub mint_b: Box<InterfaceAccount<'info, Mint>>,
  #[account(
      mut,
      associated_token::mint = mint_a,
      associated_token::authority = escrow,
      associated_token::token_program = token_program
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
  #[account(
      init_if_needed,
      payer = taker,
      associated_token::mint = mint_a,
      associated_token::authority = taker,
      associated_token::token_program = token_program
  )]
  pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,
  #[account(
      mut,
      associated_token::mint = mint_b,
      associated_token::authority = taker,
      associated_token::token_program = token_program
  )]
  pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,
  #[account(
      init_if_needed,
      payer = taker,
      associated_token::mint = mint_b,
      associated_token::authority = maker,
      associated_token::token_program = token_program
  )]
  pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Programs
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub token_program: Interface<'info, TokenInterface>,
  pub system_program: Program<'info, System>,
}

impl<'info> Take<'info> {
    fn transfer_to_maker(&mut self) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.taker_ata_b.to_account_info(),
                    to: self.maker_ata_b.to_account_info(),
                    mint: self.mint_b.to_account_info(),
                    authority: self.taker.to_account_info(),
                },
            ),
            self.escrow.receive,
            self.mint_b.decimals,
        )?;

        Ok(())
    }

    fn withdraw_and_close_vault(&mut self) -> Result<()> {
        // Create the signer seeds for the Vault
        let signer_seeds: [&[&[u8]]; 1] = [&[
            b"escrow",
            self.maker.to_account_info().key.as_ref(),
            &self.escrow.seed.to_le_bytes()[..],
            &[self.escrow.bump],
        ]];

        // Transfer Token A (Vault -> Taker)
        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.vault.to_account_info(),
                    to: self.taker_ata_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    authority: self.escrow.to_account_info(),
                },
                &signer_seeds,
            ),
            self.vault.amount,
            self.mint_a.decimals,
        )?;

        // Close the Vault
        close_account(CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            CloseAccount {
                account: self.vault.to_account_info(),
                authority: self.escrow.to_account_info(),
                destination: self.maker.to_account_info(),
            },
            &signer_seeds,
        ))?;

        Ok(())
    }
}

pub fn handler(ctx: Context<Take>) -> Result<()> {
    // Transfer Token B to Maker
    ctx.accounts.transfer_to_maker()?;

    // Withdraw and close the Vault
    ctx.accounts.withdraw_and_close_vault()?;

    Ok(())
}
```

### programs/blueshift_anchor_escrow/src/instructions/refund.rs
```rust
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

use crate::{
    errors::EscrowError,
    state::{Escrow, ESCROW_SEED},
};

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        mut,
        close = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        has_one = maker @ EscrowError::InvalidMaker,
        has_one = mint_a @ EscrowError::InvalidMintA
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(mint::token_program = token_program)]
    pub mint_a: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Refund>) -> Result<()> {
    let vault_amount = ctx.accounts.vault.amount;

    let escrow = &ctx.accounts.escrow;
    let seed_bytes = escrow.seed.to_le_bytes();
    let signer_seeds: &[&[u8]] = &[
        ESCROW_SEED,
        escrow.maker.as_ref(),
        seed_bytes.as_ref(),
        &[escrow.bump],
    ];
    let signer = &[signer_seeds];

    if vault_amount > 0 {
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint_a.to_account_info(),
                    to: ctx.accounts.maker_ata_a.to_account_info(),
                    authority: ctx.accounts.escrow.to_account_info(),
                },
                signer,
            ),
            vault_amount,
            ctx.accounts.mint_a.decimals,
        )?;
    }

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.maker.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        },
        signer,
    ))?;

    Ok(())
}
```

### programs/blueshift_anchor_escrow/Cargo.toml
```toml
[package]
name = "blueshift_anchor_escrow"
version = "0.1.0"
description = "Created with Anchor"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]
name = "blueshift_anchor_escrow"

[features]
default = []
cpi = ["no-entrypoint"]
no-entrypoint = []
no-idl = []
no-log-ix-name = []
idl-build = ["anchor-lang/idl-build", "anchor-spl/idl-build"]
anchor-debug = []
custom-heap = []
custom-panic = []

[dependencies]
anchor-lang = { version = "0.32.1", features = ["init-if-needed"] }
anchor-spl = "0.32.1"

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(target_os, values("solana"))'] }
```

### Anchor.toml
```toml
[toolchain]
package_manager = "yarn"

[features]
resolution = true
skip-lint = false

[programs.localnet]
blueshift_anchor_escrow = "22222222222222222222222222222222222222222222"

[registry]
url = "https://api.apr.dev"

[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"

[scripts]
test = "yarn run ts-mocha -p ./tsconfig.json -t 1000000 \"tests/**/*.ts\""
```

## 4. 账户顺序（非常重要）

这些顺序必须与 struct 中字段顺序一致，否则测试可能失败。

### Make
```
maker
escrow
mint_a
mint_b
maker_ata_a
vault
associated_token_program
token_program
system_program
```

### Take
```
taker
maker
escrow
mint_a
mint_b
vault
taker_ata_a
taker_ata_b
maker_ata_b
associated_token_program
token_program
system_program
```

### Refund
```
maker
escrow
mint_a
vault
maker_ata_a
associated_token_program
token_program
system_program
```

## 5. 账户推导与说明

- Escrow PDA：
  ```
  PDA = findProgramAddress(["escrow", maker, seed_le_u64])
  ```
- Vault ATA（escrow 作为 owner，必须 allowOwnerOffCurve = true）：
  ```
  ATA = getAssociatedTokenAddressSync(mintA, escrowPda, true, token_program)
  ```

## 6. 调用顺序（从 0 到成功）

1) `anchor build`  
2) 部署（或按作业平台流程）  
3) 创建 `mint_a` / `mint_b` 并给 `maker_ata_a` 充足余额  
4) 调用 `make`：创建 escrow + vault，并转入 Token A  
5) 选择 `take` 或 `refund`  
   - `take`：交换完成并关闭 vault + escrow  
   - `refund`：退回 Token A 并关闭 vault + escrow  

## 7. 关键注意点

- `token_program` 必须与 mint owner 匹配（SPL Token vs Token-2022）
- `vault` 必须是 escrow 的 ATA，不是 maker 的 ATA
- `refund` 只能在 `take` 之前执行（`take` 会关闭 vault）
- `make` 参数顺序以 `lib.rs` 为准（当前是 `seed, deposit, receive`）。  
  `make.rs` 的 handler 按 `seed, receive, amount` 使用参数，因此测试里用 `.make(seed, receiveAmount, depositAmount)` 传参。

## 8. 常见报错与修正

- **DeclaredProgramIdMismatch**  
  同步 `declare_id!` 与 `Anchor.toml` 的 program id。

- **AccountNotInitialized (mint/vault)**  
  mint 必须是已初始化的 SPL/2022 mint；  
  vault 必须是 escrow 的 ATA，且 `make` 成功后再调用 `refund`/`take`。

- **AccountOwnedByWrongProgram**  
  传错了 `token_program`（SPL vs 2022）。

- **ConstraintSeeds (vault)**  
  vault 地址必须严格等于 `ATA(mint_a, escrowPda, true)`。

- **AccountNotEnoughKeys**  
  账户顺序与 IDL 不一致或缺失 system/associated/token program。
