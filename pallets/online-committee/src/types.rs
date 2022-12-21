use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use dbc_support::{
    machine_type::CommitteeUploadInfo,
    verify_online::{MachineConfirmStatus, OCBookResultType, Summary},
    MachineId,
};
use frame_support::ensure;
use generic_func::ItemList;
use sp_runtime::RuntimeDebug;
use sp_std::{cmp, ops, vec::Vec};

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;
/// Each committee have 480 blocks (4 hours) to verify machine
pub const DURATIONPERCOMMITTEE: u32 = 480;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;
pub const TWO_DAY: u32 = 5760;

#[derive(Clone, Debug)]
pub struct VerifySequence<AccountId> {
    pub who: AccountId,
    pub index: Vec<usize>,
}

/// Query distributed machines by committee address
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCCommitteeMachineList {
    /// machines, that distributed to committee, and should be verified
    pub booked_machine: Vec<MachineId>,
    /// machines, have submited machine info hash
    pub hashed_machine: Vec<MachineId>,
    /// machines, have submited raw machine info
    pub confirmed_machine: Vec<MachineId>,
    /// machines, online successfully
    pub online_machine: Vec<MachineId>,
}

impl OCCommitteeMachineList {
    pub fn submit_hash(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::add_item(&mut self.hashed_machine, machine_id);
    }

    pub fn submit_raw(&mut self, machine_id: MachineId) -> Result<(), CustomErr> {
        ensure!(self.hashed_machine.binary_search(&machine_id).is_ok(), CustomErr::NotSubmitHash);
        ensure!(
            self.confirmed_machine.binary_search(&machine_id).is_err(),
            CustomErr::AlreadySubmitRaw
        );

        ItemList::rm_item(&mut self.hashed_machine, &machine_id);
        ItemList::add_item(&mut self.confirmed_machine, machine_id);
        Ok(())
    }

    // 将要重新派单的机器从订单里清除
    pub fn revert_book(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, machine_id);
        ItemList::rm_item(&mut self.hashed_machine, machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, machine_id);
    }

    // 机器成功上线后，从其他字段中清理掉机器记录
    // (如果未完成某一阶段的任务，机器ID将记录在那个阶段，需要进行清理)
    pub fn online_cleanup(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, machine_id);
        ItemList::rm_item(&mut self.hashed_machine, machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, machine_id);
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    NotInBookList,
    TimeNotAllow,
    AlreadySubmitHash,
    AlreadySubmitRaw,
    NotSubmitHash,
    Overflow,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::NotInBookList => Error::NotInBookList,
            CustomErr::TimeNotAllow => Error::TimeNotAllow,
            CustomErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            CustomErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            CustomErr::NotSubmitHash => Error::NotSubmitHash,
            CustomErr::Overflow => Error::Overflow,
        }
    }
}

/// Machines' verifying committee
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCMachineCommitteeList<AccountId, BlockNumber> {
    /// When order distribution happened
    pub book_time: BlockNumber,
    /// Committees, get the job to verify machine info
    pub booked_committee: Vec<AccountId>,
    /// Committees, have submited machine info hash
    pub hashed_committee: Vec<AccountId>,
    /// When committee can submit raw machine info, submit machine info can
    /// immediately start after all booked_committee submit hash
    pub confirm_start_time: BlockNumber,
    /// Committees, have submit raw machine info
    pub confirmed_committee: Vec<AccountId>,
    /// Committees, get a consensus, so can get rewards after machine online
    pub onlined_committee: Vec<AccountId>,
    /// Current order status
    pub status: OCVerifyStatus,
}

impl<AccountId, BlockNumber> OCMachineCommitteeList<AccountId, BlockNumber>
where
    AccountId: Clone + Ord,
    BlockNumber: Copy + PartialOrd + ops::Add<Output = BlockNumber> + From<u32>,
{
    pub fn submit_hash(&mut self, committee: AccountId) -> Result<(), CustomErr> {
        ensure!(self.booked_committee.binary_search(&committee).is_ok(), CustomErr::NotInBookList);
        ensure!(
            self.hashed_committee.binary_search(&committee).is_err(),
            CustomErr::AlreadySubmitHash
        );

        ItemList::add_item(&mut self.hashed_committee, committee);
        // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
        if self.booked_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::SubmittingRaw;
        }

        Ok(())
    }

    pub fn submit_raw(&mut self, time: BlockNumber, committee: AccountId) -> Result<(), CustomErr> {
        if self.status != OCVerifyStatus::SubmittingRaw {
            ensure!(time >= self.confirm_start_time, CustomErr::TimeNotAllow);
            ensure!(time <= self.book_time + SUBMIT_RAW_END.into(), CustomErr::TimeNotAllow);
        }
        ensure!(self.hashed_committee.binary_search(&committee).is_ok(), CustomErr::NotSubmitHash);

        ItemList::add_item(&mut self.confirmed_committee, committee);
        if self.confirmed_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::Summarizing;
        }
        Ok(())
    }

    // 是Summarizing的状态或 是SummitingRaw 且在有效时间内
    pub fn can_summary(&mut self, now: BlockNumber) -> bool {
        matches!(self.status, OCVerifyStatus::Summarizing) ||
            matches!(self.status, OCVerifyStatus::SubmittingRaw) &&
                now >= self.book_time + SUBMIT_RAW_END.into()
    }

    // 记录没有提交原始信息的委员会
    pub fn summary_unruly(&self) -> Vec<AccountId> {
        let mut unruly = Vec::new();
        for a_committee in self.booked_committee.clone() {
            if self.confirmed_committee.binary_search(&a_committee).is_err() {
                ItemList::add_item(&mut unruly, a_committee);
            }
        }
        unruly
    }

    pub fn after_summary(&mut self, summary_result: MachineConfirmStatus<AccountId>) {
        match summary_result {
            MachineConfirmStatus::Confirmed(summary) => {
                self.status = OCVerifyStatus::Finished;
                self.onlined_committee = summary.valid_support;
            },
            MachineConfirmStatus::NoConsensus(_) => {},
            MachineConfirmStatus::Refuse(_) => {
                self.status = OCVerifyStatus::Finished;
            },
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCVerifyStatus {
    SubmittingHash,
    SubmittingRaw,
    Summarizing,
    Finished,
}

impl Default for OCVerifyStatus {
    fn default() -> Self {
        OCVerifyStatus::SubmittingHash
    }
}

/// A record of committee’s operations when verifying machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCCommitteeOps<BlockNumber, Balance> {
    pub staked_dbc: Balance,
    /// When one committee can start the virtual machine to verify machine info
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    /// When one committee submit raw machine info
    pub confirm_time: BlockNumber,
    pub machine_status: OCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

impl<BlockNumber, Balance> OCCommitteeOps<BlockNumber, Balance> {
    pub fn submit_hash(&mut self, time: BlockNumber, hash: [u8; 16]) {
        self.machine_status = OCMachineStatus::Hashed;
        self.confirm_hash = hash;
        self.hash_time = time;
    }

    // 添加用户对机器的操作记录
    pub fn submit_raw(&mut self, time: BlockNumber, machine_info: CommitteeUploadInfo) {
        self.confirm_time = time;
        self.machine_status = OCMachineStatus::Confirmed;
        self.machine_info = machine_info;
        self.machine_info.rand_str = Vec::new();
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCMachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for OCMachineStatus {
    fn default() -> Self {
        OCMachineStatus::Booked
    }
}

// /// What will happen after all committee submit raw machine info
// #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
// pub enum MachineConfirmStatus<AccountId> {
//     /// Machine is confirmed by committee, so can be online later
//     Confirmed(Summary<AccountId>),
//     /// Machine is refused, will not online
//     Refuse(Summary<AccountId>),
//     /// No consensus, so machine will be redistributed and verified later
//     NoConsensus(Summary<AccountId>),
// }

// impl<AccountId: Default + Clone> Default for MachineConfirmStatus<AccountId> {
//     fn default() -> Self {
//         Self::Confirmed(Summary { ..Default::default() })
//     }
// }

// impl<AccountId: Clone + Ord> MachineConfirmStatus<AccountId> {
//     // TODO: Refa it
//     pub fn get_committee_group(self) -> (Vec<AccountId>, Vec<AccountId>, Vec<AccountId>) {
//         let mut inconsistent_committee = Vec::new();
//         let mut unruly_committee = Vec::new();
//         let mut reward_committee = Vec::new();

//         match self {
//             Self::Confirmed(summary) => {
//                 unruly_committee = summary.unruly.clone();
//                 reward_committee = summary.valid_support.clone();

//                 for a_committee in summary.against {
//                     ItemList::add_item(&mut inconsistent_committee, a_committee);
//                 }
//                 for a_committee in summary.invalid_support {
//                     ItemList::add_item(&mut inconsistent_committee, a_committee);
//                 }
//             },
//             Self::NoConsensus(summary) =>
//                 for a_committee in summary.unruly {
//                     ItemList::add_item(&mut unruly_committee, a_committee);
//                 },
//             Self::Refuse(summary) => {
//                 for a_committee in summary.unruly {
//                     ItemList::add_item(&mut unruly_committee, a_committee);
//                 }
//                 for a_committee in summary.invalid_support {
//                     ItemList::add_item(&mut inconsistent_committee, a_committee);
//                 }
//                 for a_committee in summary.against {
//                     ItemList::add_item(&mut reward_committee, a_committee);
//                 }
//             },
//         }

//         (inconsistent_committee, unruly_committee, reward_committee)
//     }

//     pub fn into_book_result(&self) -> OCBookResultType {
//         match self {
//             Self::Confirmed(_) => OCBookResultType::OnlineSucceed,
//             Self::Refuse(_) => OCBookResultType::OnlineRefused,
//             Self::NoConsensus(_) => OCBookResultType::NoConsensus,
//         }
//     }

//     pub fn is_refused(&self) -> bool {
//         matches!(self, Self::Refuse(_))
//     }
// }

// #[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
// pub struct Summary<AccountId> {
//     /// Machine will be online, and those committee will get reward
//     pub valid_support: Vec<AccountId>,
//     /// Machine will be online, and those committee cannot get reward
//     /// for they submit different message from majority committee
//     pub invalid_support: Vec<AccountId>,
//     /// Committees, that not submit all message
//     /// such as: not submit hash, not submit raw info before deadline
//     pub unruly: Vec<AccountId>,
//     /// Committees, refuse machine online
//     pub against: Vec<AccountId>,
//     /// Raw machine info, most majority committee submit
//     pub info: Option<CommitteeUploadInfo>,
// }

// NOTE: If slash is from maintain committee, and reporter is slashed, but when
// committee support the reporter's slash is canceled, reporter's slash is not canceled at the same
// time. Mainwhile, if reporter's slash is canceled..
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub machine_id: MachineId,
    pub machine_stash: AccountId,
    pub stash_slash_amount: Balance,

    // TODO: maybe should record slash_reason: refuse online refused or change hardware
    // info refused, maybe slash amount is different
    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,

    pub book_result: OCBookResultType,
    pub slash_result: OCSlashResult,
}

impl<AccountId: PartialEq + cmp::Ord, BlockNumber, Balance>
    OCPendingSlashInfo<AccountId, BlockNumber, Balance>
{
    pub fn applicant_is_stash(&self, stash: AccountId) -> bool {
        self.book_result == OCBookResultType::OnlineRefused && self.machine_stash == stash
    }

    pub fn applicant_is_committee(&self, applicant: &AccountId) -> bool {
        self.inconsistent_committee.binary_search(applicant).is_ok()
    }
}

// #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
// pub enum OCBookResultType {
//     OnlineSucceed,
//     OnlineRefused,
//     NoConsensus,
//     // TODO: May add if is reonline
// }

// impl Default for OCBookResultType {
//     fn default() -> Self {
//         Self::OnlineRefused
//     }
// }

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum OCSlashResult {
    Pending,
    Canceled,
    Executed,
}

impl Default for OCSlashResult {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
