#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

use dbc_support::report::CustomErr as ReportErr;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    NotInBookList,
    TimeNotAllow,
    AlreadySubmitHash,
    AlreadySubmitRaw,
    NotSubmitHash,
    NotAllowedChangeMachineInfo,
    TelecomIsNull,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::NotInBookList => Error::NotInBookList,
            CustomErr::TimeNotAllow => Error::TimeNotAllow,
            CustomErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            CustomErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            CustomErr::NotSubmitHash => Error::NotSubmitHash,
            CustomErr::NotAllowedChangeMachineInfo => Error::NotAllowedChangeMachineInfo,
            CustomErr::TelecomIsNull => Error::TelecomIsNull,
        }
    }
}

impl<T: Config> From<ReportErr> for Error<T> {
    fn from(err: ReportErr) -> Self {
        match err {
            ReportErr::OrderNotAllowBook => Error::ReportNotAllowBook,
            ReportErr::AlreadyBooked => Error::AlreadyBooked,
            ReportErr::NotNeedEncryptedInfo => Error::NotNeedEncryptedInfo,
            ReportErr::NotOrderReporter => Error::NotOrderReporter,
            ReportErr::OrderStatusNotFeat => Error::OrderStatusNotFeat,
            ReportErr::NotOrderCommittee => Error::NotOrderCommittee,
            ReportErr::NotInBookedList => Error::NotInBookedList,
            ReportErr::NotProperCommittee => Error::NotProperCommittee,
        }
    }
}
