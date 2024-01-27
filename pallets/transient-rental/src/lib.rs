#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

use dbc_support::MachineId;
use frame_support::{
    dispatch::{DispatchResult, DispatchResultWithPostInfo},
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, OnUnbalanced, ReservableCurrency},
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{
        AccountIdConversion, CheckedAdd, CheckedMul, CheckedSub, SaturatedConversion, Saturating,
        Zero,
    },
    Perbill,
};
use sp_std::{prelude::*, str, vec::Vec};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

mod types;

pub use pallet::*;
pub use types::*;

const DLC_ASSET_ID: u32 = 88;

#[frame_support::pallet]
pub mod pallet {
    use dbc_support::traits::DLC;

    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_assets::Config {
        /// The pallet id, used for deriving its sovereign account ID.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type Dlc: DLC<AccountId = Self::AccountId, AssetId = u32, DLCBalance = u128>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    // allowed to change committee
    #[pallet::storage]
    #[pallet::getter(fn admin)]
    pub(super) type Admin<T: Config> = StorageValue<_, T::AccountId>;

    // allowed to set if miner is verified
    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn verified_miner)]
    pub(super) type VerifiedMiner<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (), ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_info)]
    pub(super) type MachineInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        types::MachineInfo<T::AccountId, T::BlockNumber>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn machine_online_locked)]
    pub(super) type MachineOnlineLocked<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u128, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn set_admin(origin: OriginFor<T>, admin: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Admin::<T>::put(admin);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn change_committee(
            origin: OriginFor<T>,
            committee: T::AccountId,
            add: bool,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed(origin)?;
            Self::ensure_admin(origin)?;
            if add {
                Committee::<T>::insert(committee, ());
            } else {
                Committee::<T>::remove(committee);
            }
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn change_verified_miner(
            origin: OriginFor<T>,
            miner: T::AccountId,
            add: bool,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed(origin)?;
            Self::ensure_committee(origin)?;
            if add {
                VerifiedMiner::<T>::insert(miner, ());
            } else {
                VerifiedMiner::<T>::remove(miner);
            }
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(0)]
        pub fn submit_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            machine_info: types::MachineInfo<T::AccountId, T::BlockNumber>,
        ) -> DispatchResultWithPostInfo {
            let miner = ensure_signed(origin)?;
            if let Some(_) = Self::machine_info(&machine_id) {
                return Err(Error::<T>::MachineIdExist.into())
            }

            ensure!(
                <T as Config>::Dlc::get_asset_balance(DLC_ASSET_ID, &miner) >=
                    machine_info.stake_info.stake_dlc_amount,
                Error::<T>::InsufficientDLCBalance
            );

            <T as Config>::Dlc::do_transfer(
                DLC_ASSET_ID,
                &miner,
                &Self::account_id(),
                machine_info.stake_info.stake_dlc_amount,
            )?;
            MachineOnlineLocked::<T>::insert(&machine_id, machine_info.stake_info.stake_dlc_amount);

            MachineInfo::<T>::insert(machine_id, machine_info);
            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            Self::ensure_committee(renter)?;
            // T::Currency::transfer(&renter, &Self::account_id(), rent, KeepAlive)?;
            Ok(().into())
        }
    }

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ControllerStashBonded(T::AccountId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AdminNotExist,
        RequireAdmin,
        RequireCommittee,
        MachineIdExist,
        InsufficientDLCBalance,
    }
}

impl<T: Config> Pallet<T> {
    pub fn ensure_admin(who: T::AccountId) -> DispatchResultWithPostInfo {
        if let Some(admin) = Self::admin() {
            ensure!(admin == who, Error::<T>::RequireAdmin);
            Ok(().into())
        } else {
            return Err(Error::<T>::AdminNotExist.into())
        }
    }

    pub fn ensure_committee(who: T::AccountId) -> DispatchResultWithPostInfo {
        Committee::<T>::contains_key(&who)
            .then(|| ().into())
            .ok_or(Error::<T>::RequireAdmin.into())
    }

    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }
}
