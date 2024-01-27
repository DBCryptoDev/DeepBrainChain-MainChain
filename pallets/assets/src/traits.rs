use crate::{Config, Pallet, TransferFlags};
use dbc_support::traits::DLC;
use frame_support::dispatch::DispatchError;

impl<T: Config> DLC for Pallet<T> {
    type AssetId = T::AssetId;
    type AccountId = T::AccountId;
    type DLCBalance = T::Balance;

    fn get_asset_balance(asset_id: Self::AssetId, who: &Self::AccountId) -> Self::DLCBalance {
        return Self::balance(asset_id, who)
    }

    fn do_transfer(
        asset_id: Self::AssetId,
        from: &Self::AccountId,
        to: &Self::AccountId,
        amount: Self::DLCBalance,
    ) -> Result<Self::DLCBalance, DispatchError> {
        let f = TransferFlags { keep_alive: false, best_effort: false, burn_dust: false };
        return Self::do_transfer(asset_id, from, to, amount, None, f)
    }
}
