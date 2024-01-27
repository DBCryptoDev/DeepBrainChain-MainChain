use codec::{Decode, Encode};
#[cfg(feature = "std")]
use dbc_support::rpc_types::serde_text;
use dbc_support::MachineId;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{Saturating, Zero},
    RuntimeDebug,
};
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineInfo<AccountId: Ord, BlockNumber> {
    pub machine_owner: AccountId,
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub machine_id: MachineId,

    pub hardware_info: HardwareInfo,
    pub rent_info: RentInfo<BlockNumber>,
    pub stake_info: StakeInfo,
    pub internet_cafe_info: InternetCafeInfo,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct HardwareInfo {
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub gpu_type: Vec<u8>, // GPU型号
    pub gpu_num: u32,   // GPU数量
    pub cuda_core: u32, // CUDA core数量
    pub gpu_mem: u64,   // GPU显存

    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub cpu_type: Vec<u8>, // CPU型号
    pub cpu_core_num: u32, // CPU内核数
    pub cpu_rate: u64,     // CPU频率

    pub mem_num: u64, // 内存大小

    pub sys_disk: u64,  // 系统盘大小
    pub data_disk: u64, // 数据盘大小

    pub calc_point: u64, // 算力值
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RentInfo<BlockNumber> {
    pub rent_price: u64,               // 每小时价格(单位USD)
    pub rent_duration: BlockNumber,    // 每次可出租时长
    pub rent_start_block: BlockNumber, // 可出租开始区块
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StakeInfo {
    pub stake_multiple: u64,    // 选择的质押倍数或者不质押
    pub stake_dlc_amount: u128, // 质押的dlc数量, 上线时质押一定的DLC
}

// 网吧信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct InternetCafeInfo {
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub internet_cafe_id: Vec<u8>,
}
