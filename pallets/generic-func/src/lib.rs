#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::*,
    traits::{Randomness, Currency, LockableCurrency, OnUnbalanced, ExistenceRequirement::AllowDeath}
};
use frame_system::pallet_prelude::*;
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, RandomNumberGenerator};
use sp_std::prelude::*;

pub use pallet::*;

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type BlockPerEra: Get<u32>;
        type RandomnessSource: Randomness<H256>;
        type FixedTxFee: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // nonce to generate random number for selecting committee
    #[pallet::type_value]
    pub(super) fn RandNonceDefault<T: Config>() -> u64 {
        0
    }

    #[pallet::storage]
    #[pallet::getter(fn rand_nonce)]
    pub(super) type RandNonce<T: Config> = StorageValue<_, u64, ValueQuery, RandNonceDefault<T>>;

    // 控制全局交易费用
    #[pallet::storage]
    #[pallet::getter(fn fixed_tx_fee)]
    pub type FixedTxFee<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 设置交易费管理者
    #[pallet::storage]
    #[pallet::getter(fn tx_fee_collector)]
    pub type TxFeeCollector<T: Config> = StorageValue<_, T::AccountId>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置租用机器手续费：10 DBC
        #[pallet::weight(0)]
        pub fn set_fixed_tx_fee(origin: OriginFor<T>, tx_fee: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            FixedTxFee::<T>::put(tx_fee);
            Ok(().into())
        }

        // 设置交易费管理人
        #[pallet::weight(0)]
        pub fn set_tx_fee_collector(origin: OriginFor<T>, who: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            TxFeeCollector::<T>::put(who);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn deposit_into_treasury(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if !<T as pallet::Config>::Currency::can_slash(&who, amount) {return Err(Error::<T>::FreeBalanceNotEnough.into());
            }

            let (imbalance, _) = <T as pallet::Config>::Currency::slash(&who, amount);
            Self::deposit_event(Event::DonateToTreasury(who, amount));
            T::FixedTxFee::on_unbalanced(imbalance);
            Ok(().into())
        }

        // TODO: 销毁某个账户的DBC
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        DonateToTreasury(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        FreeBalanceNotEnough,
    }
}

impl<T: Config> Pallet<T> {
    // Add randomness
    fn update_nonce() -> Vec<u8> {
        let nonce = RandNonce::<T>::get();
        let nonce: u64 = if nonce == u64::MAX {
            0
        } else {
            RandNonce::<T>::get() + 1
        };
        RandNonce::<T>::put(nonce);
        nonce.encode()
    }

    // Generate random num, range: [0, max]
    pub fn random_u32(max: u32) -> u32 {
        let subject = Self::update_nonce();
        let random_seed = T::RandomnessSource::random(&subject);
        let mut rng = <RandomNumberGenerator<BlakeTwo256>>::new(random_seed);
        rng.pick_u32(max)
    }

    // 每次交易消耗一些交易费: 10DBC
    pub fn pay_fixed_tx_fee(who: T::AccountId) -> Result<(), ()> {
        let fixed_tx_fee = Self::fixed_tx_fee();

        if <T as Config>::Currency::free_balance(&who) > fixed_tx_fee {
            let tx_fee_collector = Self::tx_fee_collector().ok_or(())?;
            <T as Config>::Currency::transfer(
                &who,
                &tx_fee_collector,
                fixed_tx_fee,
                AllowDeath
            ).map_err(|_| ())?;
            return Ok(())
        }
        return Err(())
    }
}