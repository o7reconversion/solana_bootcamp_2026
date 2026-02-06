// =============================================================================
// 状态模块 - 托管账户数据结构定义
// =============================================================================
// 本模块定义了托管（Escrow）账户的数据结构，用于存储代币交换的状态信息

use pinocchio::Address;
use pinocchio::error::ProgramError;
use core::mem::size_of;

// =============================================================================
// Escrow 托管账户结构体
// =============================================================================
// 此结构体存储在链上账户的数据部分，记录一个托管交易的完整状态
//
// PDA（Program Derived Address，程序派生地址）说明：
// - PDA 是由程序 ID 和种子（seeds）派生出来的特殊地址
// - PDA 没有对应的私钥，只能由程序签名使用
// - 这里使用 PDA 作为托管账户，确保只有本程序能控制该账户
//
// #[repr(C)] 属性：
// - 确保结构体在内存中按 C 语言规则布局
// - 保证字段顺序和内存对齐与预期一致
// - 这对于序列化/反序列化非常重要
#[repr(C)]
pub struct Escrow {
    // 种子：用于派生 PDA 的随机数
    // 确保每个托管账户都有唯一的地址
    // 客户端和程序使用相同的种子 + maker + mint_a 可以派生出相同的 PDA
    pub seed: u64,

    // 创建者：发起托管交易的用户地址
    // 用于验证只有创建者才能执行退款操作
    pub maker: Address,

    // 代币 A 的 mint 地址：被存入金库的代币类型
    // 例如：如果是 SOL，则是 SOL 的 mint 地址
    pub mint_a: Address,

    // 代币 B 的 mint 地址：创建者希望获得的代币类型
    // 接受者需要发送这个类型的代币来完成交易
    pub mint_b: Address,

    // 期望数量：创建者希望获得的代币 B 的数量
    // 接受者必须发送至少这个数量的代币 B 才能接受交易
    pub receive: u64,

    // Bump 种子：PDA 派生时找到的有效 bump 值
    // Solana 使用 "find_program_address" 查找 PDA，会返回一个 bump 值
    // 验证签名时需要提供这个 bump 值（通常追加在 seeds 后面）
    // 使用 [u8; 1] 而不是 u8 是为了确保内存布局
    pub bump: [u8;1]
}

// =============================================================================
// Escrow 结构体的方法实现
// =============================================================================
impl Escrow {
    // ------------------------------------------------------------------------
    // 常量：账户数据长度
    // ------------------------------------------------------------------------
    // 这是 Escrow 结构体在链上账户中占用的总字节数
    // 计算方式：每个字段的大小之和
    // - u64: 8 字节
    // - Address: 32 字节
    // - [u8; 1]: 1 字节
    // 总计：8 + 32 + 32 + 32 + 8 + 1 = 113 字节
    //
    // 用途：创建账户时需要指定空间大小，客户端和程序都需要知道这个值
    pub const LEN: usize = size_of::<u64>()                     // seed: 8 字节
        + size_of::<Address>()                                  // maker: 32 字节
        + size_of::<Address>()                                  // mint_a: 32 字节
        + size_of::<Address>()                                  // mint_b: 32 字节
        + size_of::<u64>()                                      // receive: 8 字节
        + size_of::<[u8;1]>();                                  // bump: 1 字节

    // ------------------------------------------------------------------------
    // 加载可变引用
    // ------------------------------------------------------------------------
    // 从字节数组（账户数据）中加载 Escrow 结构体的可变引用
    //
    // 参数：
    //   bytes: 账户数据的可变字节数组切片
    //
    // 返回：
    //   成功：返回 Escrow 的可变引用
    //   失败：返回 InvalidAccountData 错误
    //
    // 安全性：
    //   使用 unsafe 代码块和 transmute 将字节指针转换为结构体指针
    //   这是因为我们需要直接操作原始内存，避免复制开销
    //   前提条件：字节数组必须足够大且内存布局正确
    //
    // #[inline(always)]:
    //   强制编译器内联此函数，消除函数调用开销
    //   对于这种小型辅助函数，内联能提高性能
    #[inline(always)]
    pub fn load_mut(bytes: &mut [u8]) -> Result<&mut Self, ProgramError> {
        // 验证字节长度是否匹配
        if bytes.len() != Escrow::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // 将字节指针转换为 Escrow 指针，然后解引用为可变引用
        Ok(unsafe { &mut *core::mem::transmute::<*mut u8, *mut Self>(bytes.as_mut_ptr()) })
    }

    // ------------------------------------------------------------------------
    // 加载只读引用
    // ------------------------------------------------------------------------
    // 从字节数组（账户数据）中加载 Escrow 结构体的只读引用
    //
    // 参数：
    //   bytes: 账户数据的只读字节数组切片
    //
    // 返回：
    //   成功：返回 Escrow 的只读引用
    //   失败：返回 InvalidAccountData 错误
    //
    // 用途：
    //   当只需要读取账户数据而不需要修改时使用此方法
    //   例如：验证托管状态、检查创建者等
    #[inline(always)]
    pub fn load(bytes: &[u8]) -> Result<&Self, ProgramError> {
        // 验证字节长度是否匹配
        if bytes.len() != Escrow::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // 将只读字节指针转换为只读 Escrow 指针，然后解引用为引用
        Ok(unsafe { &*core::mem::transmute::<*const u8, *const Self>(bytes.as_ptr()) })
    }

    // ------------------------------------------------------------------------
    // Setter 方法：设置各个字段
    // ------------------------------------------------------------------------
    // 以下方法用于设置 Escrow 结构体的各个字段
    // 使用 #[inline(always)] 确保这些简单的赋值操作被内联，消除函数调用开销
    //
    // 为什么需要这些 setter 方法？
    // - Pinocchio 不像 Anchor 那样自动实现序列化
    // - 需要手动提供方法来修改结构体字段
    // - 提供一致的 API 接口

    #[inline(always)]
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    #[inline(always)]
    pub fn set_maker(&mut self, maker: Address) {
        self.maker = maker;
    }

    #[inline(always)]
    pub fn set_mint_a(&mut self, mint_a: Address) {
        self.mint_a = mint_a;
    }

    #[inline(always)]
    pub fn set_mint_b(&mut self, mint_b: Address) {
        self.mint_b = mint_b;
    }

    #[inline(always)]
    pub fn set_receive(&mut self, receive: u64) {
        self.receive = receive;
    }

    #[inline(always)]
    pub fn set_bump(&mut self, bump: [u8;1]) {
        self.bump = bump;
    }

    // ------------------------------------------------------------------------
    // 批量设置方法
    // ------------------------------------------------------------------------
    // 一次性设置所有字段，避免多次函数调用
    //
    // 参数：
    //   seed: PDA 派生种子
    //   maker: 创建者地址
    //   mint_a: 存入的代币 mint 地址
    //   mint_b: 请求的代币 mint 地址
    //   receive: 请求的代币数量
    //   bump: PDA bump 种子
    //
    // 用途：
    //   在创建托管账户时，一次性初始化所有字段
    //   比逐个调用 setter 方法更高效
    #[inline(always)]
    pub fn set_inner(&mut self, seed: u64, maker: Address, mint_a: Address, mint_b: Address, receive: u64, bump: [u8;1]) {
        self.seed = seed;
        self.maker = maker;
        self.mint_a = mint_a;
        self.mint_b = mint_b;
        self.receive = receive;
        self.bump = bump;
    }
}