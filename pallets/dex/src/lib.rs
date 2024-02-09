#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::fungibles;
use frame_support::{ensure, Blake2_128Concat, DebugNoBound, PalletId};
use scale_info::TypeInfo;
use sp_runtime::traits::{CheckedDiv, CheckedMul, IntegerSquareRoot, Zero};

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// The hasher used by the pallet's storage
pub type Hasher = Blake2_128Concat;

/// Alias for the asset balance type
pub type AssetBalanceOf<T> = <<T as Config>::Fungibles as fungibles::Inspect<
	<T as frame_system::Config>::AccountId,
>>::Balance;

/// Represents an amount of a specific asset in the DEX.
///
/// Each instance of `AssetAmount` includes the asset identifier (`asset_id`)
/// and the balance of that asset (`balance`).
#[derive(Clone, Copy, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AssetAmount<T: Config> {
	asset_id: T::DexAssetId,
	balance: AssetBalanceOf<T>,
}

impl<T: Config> AssetAmount<T> {
	/// Creates a new `AssetAmount` instance.
	///
	/// # Arguments
	///
	/// * `asset_id` - A unique identifier for the asset.
	/// * `balance` - The balance of the asset.
	pub fn new(asset_id: T::DexAssetId, balance: AssetBalanceOf<T>) -> Self {
		Self { asset_id, balance }
	}
}

/// Represents a pair of asset identifiers in the DEX.
///
/// This struct is used to identify a liquidity pool for a pair of assets.
#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AssetIdPair<T: Config> {
	asset_x_id: T::DexAssetId,
	asset_y_id: T::DexAssetId,
}

impl<T: Config> AssetIdPair<T> {
	/// Creates a new `AssetIdPair`.
	///
	/// # Arguments
	///
	/// * `asset_x_id` - Identifier for the first asset.
	/// * `asset_y_id` - Identifier for the second asset.
	///
	/// # Errors
	///
	/// Returns `Error::<T>::InvalidPair` if the asset identifiers are the same.
	pub fn new(asset_x_id: T::DexAssetId, asset_y_id: T::DexAssetId) -> Result<Self, Error<T>> {
		ensure!(&asset_x_id != &asset_y_id, Error::<T>::InvalidPair);
		Ok(Self {
			asset_x_id: asset_x_id.clone().min(asset_y_id.clone()),
			asset_y_id: asset_x_id.max(asset_y_id),
		})
	}
}

/// Represents a pair of asset amounts.
///
/// Used for operations involving two different assets, such as providing liquidity
/// or performing asset swaps.
#[derive(Clone, Copy, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AssetAmountPair<T: Config> {
	amount_x: AssetAmount<T>,
	amount_y: AssetAmount<T>,
}

// todo: maybe just x,y? amount is usless word

impl<T: Config> AssetAmountPair<T> {
	/// Creates an empty `AssetAmountPair` with zero balances.
	///
	/// # Arguments
	///
	/// * `asset_id_pair` - Pair of asset identifiers.
	pub fn empty(asset_id_pair: AssetIdPair<T>) -> Self {
		Self {
			amount_x: AssetAmount {
				asset_id: asset_id_pair.asset_x_id,
				balance: AssetBalanceOf::<T>::zero(),
			},
			amount_y: AssetAmount {
				asset_id: asset_id_pair.asset_y_id,
				balance: AssetBalanceOf::<T>::zero(),
			},
		}
	}

	//todo, same here, very verbose names for asset_x, asset_y

	/// Creates a new `AssetAmountPair` with specified amounts.
	///
	/// # Arguments
	///
	/// * `asset_id_pair` - Pair of asset identifiers.
	/// * `amount_x` - Amount for the first asset.
	/// * `amount_y` - Amount for the second asset.
	pub fn new(
		asset_id_pair: AssetIdPair<T>,
		amount_x: AssetBalanceOf<T>,
		amount_y: AssetBalanceOf<T>,
	) -> Self {
		Self {
			amount_x: AssetAmount { asset_id: asset_id_pair.asset_x_id, balance: amount_x },
			amount_y: AssetAmount { asset_id: asset_id_pair.asset_y_id, balance: amount_y },
		}
	}

	fn id(&self) -> Result<AssetIdPair<T>, Error<T>> {
		let pair = self.clone();
		AssetIdPair::new(pair.amount_x.asset_id, pair.amount_y.asset_id)
	}
}

/// Represents a liquidity pool in the DEX.
///
/// A liquidity pool consists of two assets and their respective amounts, total liquidity,
/// and an identifier for the liquidity provider token.
#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LiquidityPool<T: Config> {
	asset_amounts: AssetAmountPair<T>,
	total_liquidity: AssetBalanceOf<T>,
	lp_token_id: T::DexAssetId,
}

impl<T: Config> LiquidityPool<T> {
	/// Creates an empty liquidity pool from a given asset pair and LP token identifier.
	///
	/// # Arguments
	///
	/// * `liquidity_id_pair` - Pair of asset identifiers for the pool.
	/// * `lp_token_id` - Identifier for the liquidity provider token.
	fn empty_from_pair(liquidity_id_pair: AssetIdPair<T>, lp_token_id: T::DexAssetId) -> Self {
		Self {
			asset_amounts: AssetAmountPair::empty(liquidity_id_pair),
			total_liquidity: AssetBalanceOf::<T>::zero(),
			lp_token_id,
		}
	}
}

const PALLET_ID: PalletId = PalletId(*b"__Dex__!");

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use codec::EncodeLike;
	use frame_support::traits::fungibles::{Create, Inspect, Mutate};
	use frame_support::traits::tokens::Fortitude::Force;
	use frame_support::traits::tokens::{Precision, Preservation};
	use frame_support::{
		pallet_prelude::*,
		traits::fungible::{self},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{
		AccountIdConversion, Convert, EnsureAdd, EnsureDiv, EnsureMul, EnsureSub,
	};
	use sp_runtime::{ArithmeticError, FixedU128, Perbill, Saturating};

	use crate::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Native Balance Type
		type NativeBalance: fungible::Inspect<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::hold::Mutate<Self::AccountId>
			+ fungible::hold::Inspect<Self::AccountId>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId>;

		/// The Assets Balance Type
		type Fungibles: Inspect<Self::AccountId, AssetId = Self::DexAssetId>
			+ Mutate<Self::AccountId>
			+ Create<Self::AccountId>;

		/// Type to use for asset IDs, needs to implement `Ord` to prevent duplicate asset liquidity pool ids
		type DexAssetId: Ord
			+ Clone
			+ Copy
			+ PartialEq
			+ TypeInfo
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaxEncodedLen
			+ Debug;

		/// The minimum balance for LP tokens
		type LpTokenDust: Get<AssetBalanceOf<Self>>;

		/// The swap fee percentage
		type FeePct: Get<Perbill>;

		/// Type to convert two asset balances to a ratio
		type AssetBalancePairToRatioConverter: Convert<
			(AssetBalanceOf<Self>, AssetBalanceOf<Self>),
			FixedU128,
		>;
	}

	#[pallet::storage]
	pub type Assets<T: Config> = StorageMap<_, Hasher, T::DexAssetId, AssetBalanceOf<T>>;

	#[pallet::storage]
	pub type Pools<T>
	where
		T: Config + TypeInfo,
	= StorageMap<_, Hasher, AssetIdPair<T>, LiquidityPool<T>>;

	// todo remove the comment below

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Liquidity token created for a new pool
		LpTokenCreated { lp_token_id: T::DexAssetId },

		/// Liquidity tokens issues to account.
		LiquidityProvided {
			who: T::AccountId,
			provided: AssetAmountPair<T>,
			lp_tokens: AssetBalanceOf<T>,
		},

		/// Liquidity tokens issues to account
		LiquidityRemoved {
			who: T::AccountId,
			removed: AssetAmountPair<T>,
			lp_tokens: AssetBalanceOf<T>,
		},

		/// Token swapped by account.
		TokenSwapped { who: T::AccountId, give: AssetAmount<T>, take: AssetAmount<T> },

		/// Asset price
		AssetPrice { price: FixedU128 },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// An arithmetic error has occurred
		Arithmetic,

		/// The provided liquidity pair is invalid
		InvalidPair,

		/// Liquidity pool does not exist
		PoolDoesntExists,

		/// Liquidity pool already exists
		PoolAlreadyExists,

		/// Insufficient liquidity provided
		InsufficientLiquidityProvided,

		/// Zero swap amount requested, amount must be positive
		ZeroSwapAmountRequested,

		/// There aren't enough asset to satisfy the swap
		SwapCannotBeSatisfied,

		/// The minimum expected swap output wasn't reached
		MinimumOutputNotReached,

		/// The maximum expected swap input was exceeded
		MaximumInputExceeded,

		/// The provided liquidity amount can lead to immediate arbitrage,
		/// provisions should conform to:
		///
		/// `x / y != dx / dy`
		///
		/// where `x` and `y` are the asset balances
		/// and `dx` and `dy` are the provision amounts
		ImmediateArbitrage,
	}

	impl<T: Config> From<ArithmeticError> for Error<T> {
		fn from(_: ArithmeticError) -> Self {
			Self::Arithmetic
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new liquidity pool with specified asset pairs and LP token.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `asset_x_id` - Identifier of the first asset.
		/// * `asset_y_id` - Identifier of the second asset.
		/// * `lp_token_id` - Identifier for the LP token.
		///
		/// # Errors
		///
		/// Returns `PoolAlreadyExists` if the pool for the given asset pair already exists.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::default())]
		pub fn create_pool(
			origin: OriginFor<T>,
			asset_x_id: T::DexAssetId,
			asset_y_id: T::DexAssetId,
			lp_token_id: T::DexAssetId,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let pair: AssetIdPair<T> = AssetIdPair::new(asset_x_id, asset_y_id)?;
			ensure!(!Pools::contains_key(&pair), Error::<T>::PoolAlreadyExists);

			Pools::<T>::insert(pair.clone(), Self::new_empty_pool(pair, &lp_token_id)?);
			Self::deposit_event(Event::LpTokenCreated { lp_token_id });
			Ok(())
		}

		/// Provide liquidity to a pool and receive LP tokens in return.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `provision` - Asset amounts to provide as liquidity.
		/// * `lp_token_id` - Identifier for the LP token.
		///
		/// # Errors
		///
		/// Returns `InsufficientLiquidityProvided` if the provided liquidity is zero for either asset.
		/// Returns `ImmediateArbitrage` if the provided liquidity can lead to immediate arbitrage.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::default())]
		pub fn provide_liquidity(
			origin: OriginFor<T>,
			provision: AssetAmountPair<T>,
			lp_token_id: T::DexAssetId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				!provision.amount_x.balance.is_zero() && !provision.amount_y.balance.is_zero(),
				Error::<T>::InsufficientLiquidityProvided
			);

			let mut pool =
				Pools::<T>::get(&provision.id()?).ok_or(Error::<T>::PoolAlreadyExists)?;

			if !pool.asset_amounts.amount_x.balance.is_zero()
				&& !pool.asset_amounts.amount_y.balance.is_zero()
			{
				ensure!(
					pool.asset_amounts
						.amount_x
						.balance
						.checked_mul(&provision.amount_x.balance)
						.ok_or(Error::<T>::Arithmetic)?
						== pool
							.asset_amounts
							.amount_y
							.balance
							.checked_mul(&provision.amount_y.balance)
							.ok_or(Error::<T>::Arithmetic)?,
					Error::<T>::ImmediateArbitrage
				);
			}

			// Transfer assets to the DEX account.
			T::Fungibles::transfer(
				provision.amount_x.asset_id.clone(),
				&who,
				&Self::dex_account_id(),
				provision.amount_x.balance,
				Preservation::Preserve,
			)?;
			T::Fungibles::transfer(
				provision.amount_y.asset_id.clone(),
				&who,
				&Self::dex_account_id(),
				provision.amount_y.balance,
				Preservation::Preserve,
			)?;

			let lp_tokens = Self::calculate_tokens_to_mint(&provision, &pool)?;

			T::Fungibles::mint_into(lp_token_id, &who, lp_tokens)?;
			Self::deposit_event(Event::LiquidityProvided {
				who,
				provided: provision.clone(),
				lp_tokens,
			});

			pool.asset_amounts.amount_x.balance += provision.amount_x.balance;
			pool.asset_amounts.amount_y.balance += provision.amount_y.balance;
			pool.total_liquidity += lp_tokens;
			Pools::<T>::insert(provision.id()?, pool);

			Ok(())
		}

		/// Remove liquidity from a pool and receive the underlying assets back.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `pair_id` - Identifier of the asset pair for the liquidity pool.
		/// * `lp_tokens` - Amount of LP tokens to burn in exchange for the assets.
		///
		/// # Errors
		///
		/// Returns `PoolDoesntExists` if the specified pool does not exist.
		/// Returns `InsufficientLiquidityProvided` if the liquidity removal results in zero assets.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::default())]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			pair_id: AssetIdPair<T>,
			lp_tokens: AssetBalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pool = Pools::<T>::get(&pair_id).ok_or(Error::<T>::PoolDoesntExists)?;
			let total_liquidity = pool.total_liquidity;

			let amount_x = lp_tokens
				.ensure_mul(pool.asset_amounts.amount_x.balance)?
				.ensure_div(total_liquidity)?;
			let amount_y = lp_tokens
				.ensure_mul(pool.asset_amounts.amount_y.balance)?
				.ensure_div(total_liquidity)?;

			ensure!(
				amount_x > Zero::zero() && amount_y > Zero::zero(),
				Error::<T>::InsufficientLiquidityProvided
			);

			// Transfer the assets back to the user.
			T::Fungibles::transfer(
				pool.asset_amounts.amount_x.asset_id.clone(),
				&Self::dex_account_id(),
				&who,
				amount_x,
				Preservation::Preserve,
			)?;
			T::Fungibles::transfer(
				pool.asset_amounts.amount_y.asset_id.clone(),
				&Self::dex_account_id(),
				&who,
				amount_y,
				Preservation::Preserve,
			)?;

			T::Fungibles::burn_from(
				pool.lp_token_id,
				&who,
				lp_tokens,
				Precision::BestEffort,
				Force,
			)?;

			Pools::<T>::try_mutate(&pair_id, |pool| {
				if let Some(pool) = pool {
					pool.asset_amounts.amount_x.balance =
						pool.asset_amounts.amount_x.balance.saturating_sub(amount_x);
					pool.asset_amounts.amount_y.balance =
						pool.asset_amounts.amount_y.balance.saturating_sub(amount_y);
					pool.total_liquidity = pool.total_liquidity.saturating_sub(lp_tokens);
					Ok(())
				} else {
					Err(Error::<T>::PoolDoesntExists)
				}
			})?;

			Self::deposit_event(Event::<T>::LiquidityRemoved {
				who,
				removed: AssetAmountPair::<T>::new(pair_id, amount_x, amount_y),
				lp_tokens,
			});

			Ok(())
		}

		/// Perform an asset swap in a specified pool with an expected minimum take amount. if the take amount is
		/// calculated to be less than the expected minimum, the swap will fail with `MinimumOutputNotReached`.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `give` - Asset and amount to give in the swap.
		/// * `expect_min_take` - Minimum expected amount to receive from the swap.
		/// * `pool_id` - Identifier of the asset pair for the liquidity pool.
		///
		/// # Errors
		///
		/// Returns `ZeroSwapAmountRequested` if the swap amount is zero.
		/// Returns `SwapCannotBeSatisfied` if the swap cannot be satisfied with the pool's liquidity.
		/// Returns `MinimumOutputNotReached` if the output is less than the expected minimum.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::default())]
		pub fn swap_limit_take(
			origin: OriginFor<T>,
			give: AssetAmount<T>,
			expect_min_take: AssetBalanceOf<T>,
			pool_id: AssetIdPair<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!give.balance.is_zero(), Error::<T>::ZeroSwapAmountRequested);

			let mut pool = Pools::<T>::get(&pool_id).ok_or(Error::<T>::PoolDoesntExists)?;

			let (give_to, take_from) = Self::get_swap_assets(&mut pool, give.asset_id);

			let give_amount = give.balance;
			let take_amount =
				Self::calculate_swap_amounts(give_amount, give_to.balance, take_from.balance)?;

			ensure!(take_amount >= expect_min_take, Error::<T>::MinimumOutputNotReached);
			ensure!(take_amount < take_from.balance, Error::<T>::SwapCannotBeSatisfied);

			// Give to dex from user
			T::Fungibles::transfer(
				give_to.asset_id,
				&who,
				&Self::dex_account_id(),
				give_amount,
				Preservation::Preserve,
			)?;

			// Take from dex to user
			T::Fungibles::transfer(
				take_from.asset_id,
				&Self::dex_account_id(),
				&who,
				take_amount,
				Preservation::Preserve,
			)?;

			// Update pool reserves based on what was transferred
			give_to.balance = give_to.balance.ensure_add(give.balance)?;
			take_from.balance = take_from.balance.ensure_sub(take_amount)?;

			let take = AssetAmount::<T>::new(take_from.asset_id, take_amount);
			// Store updated pool
			Pools::<T>::insert(&pool_id, pool.clone());

			// Emit swap event
			Self::deposit_event(Event::<T>::TokenSwapped { who, give, take });

			Ok(())
		}

		/// Perform an asset swap in a specified pool with an maximum give amount. if the give amount is
		/// calculated to be more than the expected maximum, the swap will fail with `MinimumOutputNotReached`.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `take` - Asset and amount to take in the swap.
		/// * `expect_max_give` - Maximum expected amount to receive from the swap.
		/// * `pool_id` - Identifier of the asset pair for the liquidity pool.
		///
		/// # Errors
		///
		/// Returns `ZeroSwapAmountRequested` if the swap amount is zero.
		/// Returns `SwapCannotBeSatisfied` if the swap cannot be satisfied with the pool's liquidity.
		/// Returns `MaximumInputExceeded` if the output is less than the expected minimum.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::default())]
		pub fn swap_limit_give(
			origin: OriginFor<T>,
			take: AssetAmount<T>,
			expect_max_give: AssetBalanceOf<T>,
			pool_id: AssetIdPair<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!take.balance.is_zero(), Error::<T>::ZeroSwapAmountRequested);

			let mut pool = Pools::<T>::get(&pool_id).ok_or(Error::<T>::PoolDoesntExists)?;

			let (take_from, give_to) = Self::get_swap_assets(&mut pool, take.asset_id);

			let take_amount = take.balance;
			let give_amount =
				Self::calculate_swap_amounts(take_amount, take_from.balance, give_to.balance)?;

			ensure!(give_amount <= expect_max_give, Error::<T>::MaximumInputExceeded);

			// Give to dex from user
			T::Fungibles::transfer(
				give_to.asset_id,
				&who,
				&Self::dex_account_id(),
				give_amount,
				Preservation::Preserve,
			)?;

			// Take from dex to user
			T::Fungibles::transfer(
				take_from.asset_id,
				&Self::dex_account_id(),
				&who,
				take_amount,
				Preservation::Preserve,
			)?;

			// Update pool reserves based on what was transferred.
			give_to.balance = give_to.balance.ensure_add(give_amount)?;
			take_from.balance = take_from.balance.ensure_sub(take_amount)?;

			let give = AssetAmount::<T>::new(give_to.asset_id, give_amount);
			Pools::<T>::insert(&pool_id, pool.clone());

			// Emit swap event
			Self::deposit_event(Event::<T>::TokenSwapped { who, give, take });

			Ok(())
		}

		/// Get the price of an asset in a pool.
		///
		/// # Arguments
		///
		/// * `origin` - Origin of the transaction.
		/// * `pair` - Asset pair for the liquidity pool.
		/// * `asset_id` - Identifier of the asset for which the price is requested.
		///
		/// # Errors
		///
		/// Returns `PoolDoesntExists` if the specified pool does not exist.
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::default())]
		pub fn get_asset_price(
			origin: OriginFor<T>,
			pair: AssetIdPair<T>,
			asset_id: T::DexAssetId,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?; // we don't care who the signer is

			let pool = Pools::<T>::get(&pair).ok_or(Error::<T>::PoolDoesntExists)?;

			let price = Self::get_price_of_asset_in_pool(asset_id, &pool)?;
			Self::deposit_event(Event::<T>::AssetPrice { price });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Calculates the number of liquidity provider tokens to mint for a given provision.
		///
		/// # Arguments
		///
		/// * `added` - The provision of assets being added to the pool.
		/// * `pool` - The current state of the liquidity pool.
		///
		/// # Returns
		///
		/// Returns the number of LP tokens to mint as `AssetBalanceOf<T>`.
		///
		/// # Errors
		///
		/// Returns `Error::<T>::Arithmetic` on overflow or underflow during calculations.
		fn calculate_tokens_to_mint(
			added: &AssetAmountPair<T>,
			pool: &LiquidityPool<T>,
		) -> Result<AssetBalanceOf<T>, DispatchError> {
			let (added_x, added_y) = (added.amount_x.balance, added.amount_y.balance);
			let (reserve_x, reserve_y) =
				(pool.asset_amounts.amount_x.balance, pool.asset_amounts.amount_y.balance);

			if pool.total_liquidity.is_zero() {
				let sqrt = added_x
					.checked_mul(&added_y)
					.ok_or(Error::<T>::Arithmetic)?
					.integer_sqrt_checked()
					.ok_or(Error::<T>::Arithmetic)?;
				return Ok(sqrt);
			}

			let lp_tokens = added_x
				.checked_mul(&pool.total_liquidity)
				.ok_or(Error::<T>::Arithmetic)?
				.checked_div(&reserve_x)
				.ok_or(Error::<T>::Arithmetic)?
				.min(
					added_y
						.checked_mul(&pool.total_liquidity)
						.ok_or(Error::<T>::Arithmetic)?
						.checked_div(&reserve_y)
						.ok_or(Error::<T>::Arithmetic)?,
				);

			Ok(lp_tokens)
		}

		/// Retrieves the price ratio of a specified asset in a given liquidity pool.
		///
		/// # Arguments
		///
		/// * `asset_id` - Identifier of the asset to retrieve the price for.
		/// * `pool` - The liquidity pool to calculate the price from.
		///
		/// # Returns
		///
		/// Returns the price ratio as `FixedU128`.
		///
		/// # Errors
		///
		/// Returns `ArithmeticError` on overflow or underflow during calculations.
		fn get_price_of_asset_in_pool(
			asset_id: <T as Config>::DexAssetId,
			pool: &LiquidityPool<T>,
		) -> Result<FixedU128, ArithmeticError> {
			let price_ratio = if asset_id == pool.asset_amounts.amount_x.asset_id {
				T::AssetBalancePairToRatioConverter::convert((
					pool.asset_amounts.amount_x.balance,
					pool.asset_amounts.amount_y.balance,
				))
			} else {
				T::AssetBalancePairToRatioConverter::convert((
					pool.asset_amounts.amount_y.balance,
					pool.asset_amounts.amount_x.balance,
				))
			};

			Ok(price_ratio)
		}

		pub fn dex_account_id() -> T::AccountId {
			PALLET_ID.into_account_truncating()
		}

		fn get_swap_assets(
			pool: &mut LiquidityPool<T>,
			asset_id: T::DexAssetId,
		) -> (&mut AssetAmount<T>, &mut AssetAmount<T>) {
			if pool.asset_amounts.amount_x.asset_id == asset_id {
				(&mut pool.asset_amounts.amount_x, &mut pool.asset_amounts.amount_y)
			} else {
				(&mut pool.asset_amounts.amount_y, &mut pool.asset_amounts.amount_x)
			}
		}

		fn admin_account_id() -> T::AccountId {
			PALLET_ID.into_sub_account_truncating(*b"Admin!")
		}

		fn new_empty_pool(
			id_pair: AssetIdPair<T>,
			lp_token_id: &T::DexAssetId,
		) -> Result<LiquidityPool<T>, DispatchError> {
			T::Fungibles::create(
				lp_token_id.clone(),
				Self::admin_account_id(),
				false,
				T::LpTokenDust::get(),
			)?;
			Ok(LiquidityPool::empty_from_pair(id_pair, lp_token_id.clone()))
		}

		fn calculate_swap_amounts(
			give_balance: AssetBalanceOf<T>,
			give_to_balance: AssetBalanceOf<T>,
			take_from_balance: AssetBalanceOf<T>,
		) -> Result<AssetBalanceOf<T>, DispatchError> {
			let fee_pct = T::FeePct::get();
			let amount_in_with_fee = give_balance.ensure_sub(fee_pct * give_balance)?;
			let numerator = take_from_balance.ensure_mul(amount_in_with_fee)?;
			let denominator = give_to_balance.ensure_add(amount_in_with_fee)?;

			numerator.ensure_div(denominator).map_err(Into::into)
		}
	}
}
