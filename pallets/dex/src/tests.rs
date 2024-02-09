mod tests {
	use codec::Compact;
	use frame_support::assert_ok;
	use frame_support::pallet_prelude::Get;
	use sp_io::TestExternalities;
	use sp_runtime::traits::{EnsureAdd, EnsureDiv, EnsureMul, EnsureSub};

	use crate::{mock::*, AssetAmount, AssetAmountPair, AssetIdPair, Config, LiquidityPool, Pools};

	type TestFungibles = <Test as Config>::Fungibles;

	const ADMIN: u64 = 1;
	const ALICE: u64 = 2;
	const BOB: u64 = 3;
	const CHARLIE: u64 = 4;

	const ASSET_X: u32 = 3;
	const ASSET_Y: u32 = 4;
	const LP_TOKEN_ID: u32 = 2;

	const EXISTENTIAL_DEPOSIT: u128 = 1;
	const TEN_K: u128 = 10_000;
	const TEN_M: u128 = 10_000_000;
	const TEN_B: u128 = 10_000_000_000;
	const X_Y_ID: AssetIdPair<Test> = AssetIdPair { asset_x_id: ASSET_X, asset_y_id: ASSET_Y };

	mod unit_tests {
		mod pool_tests {
			use frame_support::{assert_noop, assert_ok};

			use crate::mock::{Dex, RuntimeOrigin, System, Test};
			use crate::tests::tests::{
				create_asset, create_empty_pool, init_test_ext, TestFungibles, ALICE, ASSET_X,
				ASSET_Y, LP_TOKEN_ID, X_Y_ID,
			};
			use crate::{AssetAmountPair, Error, Event, LiquidityPool, Pools};

			#[test]
			fn create_pool_should_work() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets
					create_asset(ASSET_X);
					create_asset(ASSET_Y);

					// pool and lp token should be minted to dex with 0 balance
					assert_ok!(Dex::create_pool(
						RuntimeOrigin::signed(ALICE),
						ASSET_X,
						ASSET_Y,
						LP_TOKEN_ID
					));
					let created_pool = Pools::get(&X_Y_ID.clone());
					let expected_pool = LiquidityPool {
						asset_amounts: AssetAmountPair::<Test>::empty(X_Y_ID.clone()),
						total_liquidity: 0,
						lp_token_id: LP_TOKEN_ID,
					};
					assert!(
						matches!(created_pool, Some(pool) if pool == expected_pool),
						"Pool should be created and empty but was {:?}",
						Pools::get(&X_Y_ID)
					);
					System::assert_last_event(
						Event::LpTokenCreated { lp_token_id: LP_TOKEN_ID }.into(),
					);
					assert_eq!(TestFungibles::balance(LP_TOKEN_ID, &Dex::dex_account_id()), 0);
				});
			}

			#[test]
			fn create_pool_should_fail_if_invalid_pair() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given same asset id for x and y
					let x = ASSET_X;
					let y = ASSET_X;

					// pool creation should fail with invalid pair error
					assert_noop!(
						Dex::create_pool(RuntimeOrigin::signed(ALICE), x, y, LP_TOKEN_ID),
						Error::<Test>::InvalidPair
					);
				});
			}

			#[test]
			fn create_pool_should_fail_if_pool_already_exists() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets
					create_asset(ASSET_X);
					create_asset(ASSET_Y);

					// and a created pool
					create_empty_pool(ASSET_X, ASSET_Y);

					// pool creation should fail,
					assert_noop!(
						Dex::create_pool(
							RuntimeOrigin::signed(ALICE),
							ASSET_X,
							ASSET_Y,
							LP_TOKEN_ID
						),
						Error::<Test>::PoolAlreadyExists
					);
				});
			}

			#[test]
			fn create_pool_should_fail_if_pool_already_exists_with_reversed_asset_ids() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets
					create_asset(ASSET_X);
					create_asset(ASSET_Y);

					// and a created pool
					create_empty_pool(ASSET_X, ASSET_Y);

					// pool creation should fail when giving the same asset ids but in reverse order
					assert_noop!(
						Dex::create_pool(
							RuntimeOrigin::signed(ALICE),
							ASSET_Y,
							ASSET_X,
							LP_TOKEN_ID
						),
						Error::<Test>::PoolAlreadyExists
					);
				});
			}
		}

		mod provide_liquidity_tests {
			use frame_support::{assert_noop, assert_ok};

			use crate::mock::{Dex, RuntimeOrigin, System, Test};
			use crate::tests::tests::{
				assert_account_has, create_asset, create_asset_amount_pair,
				create_bad_asset_amount_pair, create_balanced_pool, create_empty_pool,
				init_test_ext, mint_asset, ALICE, ASSET_X, ASSET_Y, BOB, EXISTENTIAL_DEPOSIT,
				LP_TOKEN_ID, TEN_K, TEN_M, X_Y_ID,
			};
			use crate::{Error, Event};

			#[test]
			fn provide_liquidity_works() {
				let lp = create_asset_amount_pair(TEN_M, ASSET_X, ASSET_Y);
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_asset(LP_TOKEN_ID);
					create_empty_pool(ASSET_X, ASSET_Y);

					// and assets minted to alice
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// provide liquidity should pass,
					assert_ok!(Dex::provide_liquidity(
						RuntimeOrigin::signed(ALICE),
						lp,
						LP_TOKEN_ID
					));

					// liquidity assets should be transferred to dex account,
					assert_account_has(Dex::dex_account_id(), ASSET_X, TEN_M);
					assert_account_has(Dex::dex_account_id(), ASSET_Y, TEN_M);

					// liquidity token should be minted to alice
					assert_account_has(ALICE, LP_TOKEN_ID, TEN_M);

					// and token issuance event should be emitted
					System::assert_last_event(
						Event::LiquidityProvided {
							who: ALICE,
							provided: lp.clone(),
							lp_tokens: TEN_M,
						}
						.into(),
					);
				});
			}

			#[test]
			fn provide_liquidity_second_time_works() {
				let alice_lp = create_asset_amount_pair(TEN_M, ASSET_X, ASSET_Y);
				let bob_lp = create_asset_amount_pair(TEN_K, ASSET_X, ASSET_Y);
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_asset(LP_TOKEN_ID);
					create_empty_pool(ASSET_X, ASSET_Y);

					// and assets minted to alice
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// and assets minted to bob
					mint_asset(BOB, TEN_K + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(BOB, TEN_K + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// provide liquidity by alice should pass
					assert_ok!(Dex::provide_liquidity(
						RuntimeOrigin::signed(ALICE),
						alice_lp,
						LP_TOKEN_ID
					));

					// liquidity assets should be transferred to dex account,
					assert_account_has(Dex::dex_account_id(), ASSET_X, TEN_M);
					assert_account_has(Dex::dex_account_id(), ASSET_Y, TEN_M);

					// liquidity token should be minted to alice
					assert_account_has(ALICE, LP_TOKEN_ID, TEN_M);

					// and token issuance event should be emitted
					System::assert_last_event(
						Event::LiquidityProvided {
							who: ALICE,
							provided: alice_lp.clone(),
							lp_tokens: TEN_M,
						}
						.into(),
					);

					// provide liquidity by bob should pass
					assert_ok!(Dex::provide_liquidity(
						RuntimeOrigin::signed(BOB),
						bob_lp,
						LP_TOKEN_ID
					));

					// liquidity assets should be transferred to dex account,
					assert_account_has(Dex::dex_account_id(), ASSET_X, TEN_M + TEN_K);
					assert_account_has(Dex::dex_account_id(), ASSET_Y, TEN_M + TEN_K);

					// liquidity token should be minted to bob
					assert_account_has(BOB, LP_TOKEN_ID, TEN_K);

					// and token issuance event should be emitted
					System::assert_last_event(
						Event::LiquidityProvided {
							who: BOB,
							provided: bob_lp.clone(),
							lp_tokens: TEN_K,
						}
						.into(),
					);
				});
			}

			#[test]
			fn provide_liquidity_leading_to_immediate_arbitrage_fails() {
				let bad_lp = create_bad_asset_amount_pair(TEN_M, ASSET_X, ASSET_Y);

				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_balanced_pool(X_Y_ID, TEN_K, TEN_K);

					// and assets minted to alice
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// provide liquidity should fail and immediate arbitrage error should be returned,
					assert_noop!(
						Dex::provide_liquidity(RuntimeOrigin::signed(ALICE), bad_lp, LP_TOKEN_ID),
						Error::<Test>::ImmediateArbitrage
					);
				});
			}

			#[test]
			fn provide_insufficient_liquidity_leading_to_zero_tokens_fails() {
				let zero_lp = create_asset_amount_pair(0, ASSET_X, ASSET_Y);

				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_balanced_pool(X_Y_ID, TEN_K, TEN_K);

					// and assets minted to alice
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// provide liquidity should fail and immediate arbitrage error should be returned,
					assert_noop!(
						Dex::provide_liquidity(RuntimeOrigin::signed(ALICE), zero_lp, LP_TOKEN_ID),
						Error::<Test>::InsufficientLiquidityProvided
					);
				});
			}
		}

		mod remove_liquidity {
			use frame_support::{assert_noop, assert_ok};

			use crate::mock::{Dex, RuntimeOrigin, System, Test};
			use crate::tests::tests::{
				assert_account_has, create_asset, create_asset_amount_pair, create_balanced_pool,
				init_test_ext, mint_asset, ALICE, ASSET_X, ASSET_Y, EXISTENTIAL_DEPOSIT,
				LP_TOKEN_ID, TEN_K, TEN_M, X_Y_ID,
			};
			use crate::{Error, Event};

			#[test]
			fn remove_liquidity_should_work() {
				let lp_tokens = TEN_K.into();

				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool with provided liquidity
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_asset(LP_TOKEN_ID);
					create_balanced_pool(X_Y_ID, TEN_M, TEN_M);

					// and assets minted to dex account
					mint_asset(Dex::dex_account_id(), TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(Dex::dex_account_id(), TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// and lp tokens minted to Alice
					mint_asset(ALICE, lp_tokens, LP_TOKEN_ID);

					// remove liquidity should pass
					assert_ok!(Dex::remove_liquidity(RuntimeOrigin::signed(ALICE), X_Y_ID, TEN_K));

					// liquidity assets should be transferred back to Alice,
					assert_account_has(
						Dex::dex_account_id(),
						ASSET_X,
						TEN_M + EXISTENTIAL_DEPOSIT - TEN_K,
					);
					assert_account_has(
						Dex::dex_account_id(),
						ASSET_Y,
						TEN_M + EXISTENTIAL_DEPOSIT - TEN_K,
					);

					// liquidity token should be burnt for alice
					assert_account_has(ALICE, LP_TOKEN_ID, 0);

					// and liquidity removed event should be emitted
					System::assert_last_event(
						Event::LiquidityRemoved {
							who: ALICE,
							removed: create_asset_amount_pair(TEN_K, ASSET_X, ASSET_Y),
							lp_tokens,
						}
						.into(),
					);
				});
			}

			#[test]
			fn remove_liquidity_should_fail_if_pool_doesnt_exist() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// remove liquidity should pass
					assert_noop!(
						Dex::remove_liquidity(RuntimeOrigin::signed(ALICE), X_Y_ID, 0),
						Error::<Test>::PoolDoesntExists
					);
				});
			}

			#[test]
			fn remove_liquidity_should_fail_if_amount_is_zero() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool with provided liquidity
					create_balanced_pool(X_Y_ID, TEN_M, TEN_M);

					// remove liquidity should fail with insufficient liquidity provided error
					assert_noop!(
						Dex::remove_liquidity(RuntimeOrigin::signed(ALICE), X_Y_ID, 0),
						Error::<Test>::InsufficientLiquidityProvided
					);
				});
			}
		}

		mod swap_tests {
			use frame_support::{assert_noop, assert_ok};

			use crate::mock::{Dex, RuntimeEvent, RuntimeOrigin, System, Test};
			use crate::tests::tests::{
				assert_account_has, calculate_expected_give_amount,
				calculate_expected_taken_amount, create_asset, create_balanced_pool, create_pool,
				get_account_balance, init_test_ext, mint_asset, ALICE, ASSET_X, ASSET_Y,
				EXISTENTIAL_DEPOSIT, TEN_K, TEN_M, X_Y_ID,
			};
			use crate::{AssetAmount, AssetAmountPair, Error, Event};

			#[test]
			fn swap_should_work() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);
					let reserve_x = TEN_M;
					let reserve_y = TEN_M;
					let liquidity = TEN_M;
					let give = TEN_K;

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_pool(X_Y_ID, reserve_x, reserve_x, liquidity);

					// and assets minted to Alice
					mint_asset(ALICE, give + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, EXISTENTIAL_DEPOSIT, ASSET_Y);

					// and assets minted to dex account
					mint_asset(Dex::dex_account_id(), reserve_x + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(Dex::dex_account_id(), reserve_y + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// swap should work
					let expected_take_amount =
						calculate_expected_taken_amount(give, reserve_x, reserve_y);
					let asset_amounts =
						AssetAmountPair::<Test>::new(X_Y_ID, give, expected_take_amount);
					assert_ok!(Dex::swap_limit_take(
						RuntimeOrigin::signed(ALICE),
						asset_amounts.amount_x,
						expected_take_amount,
						X_Y_ID
					));

					// and token issuance event should be emitted, with fee applied to taken amount
					System::assert_last_event(
						Event::TokenSwapped {
							who: ALICE,
							give: asset_amounts.amount_x,
							take: asset_amounts.amount_y,
						}
						.into(),
					);

					assert_account_has(ALICE, ASSET_X, EXISTENTIAL_DEPOSIT);
					assert_account_has(ALICE, ASSET_Y, EXISTENTIAL_DEPOSIT + expected_take_amount);
				});
			}

			#[test]
			fn swap_limit_give_should_work() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);
					let reserve_x = TEN_M;
					let reserve_y = TEN_M;
					let liquidity = TEN_M;
					let take = TEN_K;

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_pool(X_Y_ID, reserve_x, reserve_y, liquidity);

					// and assets minted to Alice
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// and assets minted to dex account
					mint_asset(Dex::dex_account_id(), reserve_x + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(Dex::dex_account_id(), reserve_y + EXISTENTIAL_DEPOSIT, ASSET_Y);

					let expected_max_give_amount =
						calculate_expected_give_amount(take, reserve_x, reserve_y);

					// swap should work
					let take_amount = AssetAmount::<Test>::new(ASSET_Y, take);
					assert_ok!(Dex::swap_limit_give(
						RuntimeOrigin::signed(ALICE),
						take_amount,
						expected_max_give_amount,
						X_Y_ID
					));

					// and token issuance event should be emitted
					let event = System::events().last().unwrap().clone().event;
					if let RuntimeEvent::Dex(Event::TokenSwapped { who, give, take }) = event {
						assert_eq!(who, ALICE);
						assert!(give.balance <= expected_max_give_amount);
						assert_eq!(take.balance, take_amount.balance);
					} else {
						panic!("Expected TokenSwapped event");
					}

					// and alice should have exactly expected amount of asset y increased
					assert_account_has(ALICE, ASSET_Y, TEN_M + EXISTENTIAL_DEPOSIT + take);

					// and no more than expected_max_give_amount of asset x decreased
					assert!(
						get_account_balance(ALICE, ASSET_X)
							> EXISTENTIAL_DEPOSIT + (TEN_M - expected_max_give_amount)
					);
				});
			}

			#[test]
			fn swapping_giving_zero_amount_should_fail() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					assert_noop!(
						Dex::swap_limit_take(
							RuntimeOrigin::signed(ALICE),
							AssetAmount::<Test>::new(ASSET_X, 0u128),
							0u128,
							X_Y_ID
						),
						Error::<Test>::ZeroSwapAmountRequested
					);
				});
			}

			#[test]
			fn unsatisfiable_swap_should_fail() {
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool with 10k
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_balanced_pool(X_Y_ID, TEN_K, TEN_K);

					// and assets minted to Alice, value 10m
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(ALICE, TEN_M + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// and assets minted to dex account, value 10k
					mint_asset(Dex::dex_account_id(), TEN_K + EXISTENTIAL_DEPOSIT, ASSET_X);
					mint_asset(Dex::dex_account_id(), TEN_K + EXISTENTIAL_DEPOSIT, ASSET_Y);

					// swap should fail with excessive input amount when asking to swap 10m, and expecting 1m-10k
					let give = AssetAmount::<Test>::new(ASSET_X, TEN_M);
					assert_noop!(
						Dex::swap_limit_take(
							RuntimeOrigin::signed(ALICE),
							give,
							TEN_M - TEN_K,
							X_Y_ID
						),
						Error::<Test>::MinimumOutputNotReached
					);
				});
			}
		}
		mod get_asset_price_tests {
			use frame_support::assert_ok;
			use sp_runtime::FixedU128;

			use crate::mock::{Dex, RuntimeOrigin, System};
			use crate::tests::tests::{
				create_asset, create_pool, init_test_ext, ALICE, ASSET_X, ASSET_Y, TEN_M, X_Y_ID,
			};
			use crate::Event::AssetPrice;

			#[test]
			fn get_price_of_should_work() {
				let x_vs_y = 2;
				let price_of_x_in_y = FixedU128::from_rational(x_vs_y, 1);
				init_test_ext().execute_with(|| {
					System::set_block_number(1);

					// given created assets and pool
					create_asset(ASSET_X);
					create_asset(ASSET_Y);
					create_pool(X_Y_ID, TEN_M * x_vs_y, TEN_M, TEN_M);

					// get price should work
					assert_ok!(Dex::get_asset_price(RuntimeOrigin::signed(ALICE), X_Y_ID, ASSET_X));

					// and token asset price event should be emitted
					System::assert_last_event(AssetPrice { price: price_of_x_in_y }.into());
				});
			}
		}
	}

	mod integration_tests {
		use frame_support::assert_ok;
		use sp_runtime::traits::Convert;

		use crate::Event::AssetPrice;

		use super::*;

		#[test]
		fn create_pool_add_liquidity_by_alice_swap_by_bob_get_price_check_balances_and_reward() {
			init_test_ext().execute_with(|| {
				System::set_block_number(1);

				let reserve_x = TEN_M;
				let reserve_y = TEN_M;
				let liquidity = TEN_M;
				let give = TEN_K;

				// Create assets
				create_asset(ASSET_X);
				create_asset(ASSET_Y);

				// Alice creates pool
				assert_ok!(Dex::create_pool(
					RuntimeOrigin::signed(ALICE),
					ASSET_X,
					ASSET_Y,
					LP_TOKEN_ID
				));

				// Alice Provides liquidity of 10m
				mint_asset(ALICE, liquidity + EXISTENTIAL_DEPOSIT, ASSET_X);
				mint_asset(ALICE, liquidity + EXISTENTIAL_DEPOSIT, ASSET_Y);
				let provision = create_asset_amount_pair(liquidity, ASSET_X, ASSET_Y);
				assert_ok!(Dex::provide_liquidity(
					RuntimeOrigin::signed(ALICE),
					provision,
					LP_TOKEN_ID
				));

				// Bob Swaps 10k
				mint_asset(BOB, give + EXISTENTIAL_DEPOSIT, ASSET_X);
				let give_amount = AssetAmount::<Test>::new(ASSET_X, give);
				let expected_taken_amount =
					calculate_expected_taken_amount(give_amount.balance, reserve_x, reserve_y);
				assert_ok!(Dex::swap_limit_take(
					RuntimeOrigin::signed(BOB),
					give_amount,
					expected_taken_amount,
					X_Y_ID
				));

				let precision_loss = 1;
				// Check bob x tokens have been sent and y tokens received
				assert_account_has(BOB, ASSET_X, EXISTENTIAL_DEPOSIT);
				assert_account_has(
					BOB,
					ASSET_Y,
					EXISTENTIAL_DEPOSIT + expected_taken_amount - precision_loss,
				); // one lost to precision

				// Check dex x tokens have been received and y tokens sent
				assert_account_has(
					Dex::dex_account_id(),
					ASSET_X,
					EXISTENTIAL_DEPOSIT + liquidity + give_amount.balance - precision_loss, // one lost to precision
				);
				assert_account_has(
					Dex::dex_account_id(),
					ASSET_Y,
					EXISTENTIAL_DEPOSIT + liquidity - expected_taken_amount - precision_loss, // one lost to precision
				);

				// Bob gets asset price
				let (expected_x_reserve, expected_y_reserve) = (10_010_000u128, 9_990_110u128);
				assert_ok!(Dex::get_asset_price(RuntimeOrigin::signed(BOB), X_Y_ID, ASSET_X));
				System::assert_last_event(
					AssetPrice {
						price: AssetBalancePairToRatioConverter::convert((
							expected_x_reserve,
							expected_y_reserve,
						)),
					}
					.into(),
				);

				// Get pool and check reserves have changed
				assert_eq!(
					Pools::<Test>::get(&X_Y_ID).expect("pool should exist"),
					create_pool(X_Y_ID, expected_x_reserve, expected_y_reserve, liquidity)
				);

				// Check alice lp_tokens have stayed the same
				assert_account_has(ALICE, LP_TOKEN_ID, liquidity);

				// Alice removes liquidity and is rewarded
				assert_ok!(Dex::remove_liquidity(
					RuntimeOrigin::signed(ALICE),
					X_Y_ID,
					liquidity - 1
				)
				.into());
				let total_alice_balance =
					get_account_balance(ALICE, ASSET_X) + get_account_balance(ALICE, ASSET_Y);
				assert!(total_alice_balance > liquidity + liquidity);
				assert_account_has(ALICE, LP_TOKEN_ID, EXISTENTIAL_DEPOSIT);
			});
		}

		#[test]
		fn create_pool_add_liquidity_by_alice_add_liquidity_by_charlie_swap_by_bob_get_price_check_balances(
		) {
			init_test_ext().execute_with(|| {
				System::set_block_number(1);

				let reserve_x = TEN_M;
				let reserve_y = TEN_M;
				let alice_liquidity = TEN_M;
				let charlie_liquidity = TEN_K;
				let give = TEN_K;

				// Create assets
				create_asset(ASSET_X);
				create_asset(ASSET_Y);

				// Alice creates pool
				assert_ok!(Dex::create_pool(
					RuntimeOrigin::signed(ALICE),
					ASSET_X,
					ASSET_Y,
					LP_TOKEN_ID
				));

				// Alice Provides liquidity of 10m
				mint_asset(ALICE, alice_liquidity + EXISTENTIAL_DEPOSIT, ASSET_X);
				mint_asset(ALICE, alice_liquidity + EXISTENTIAL_DEPOSIT, ASSET_Y);
				let provision = create_asset_amount_pair(alice_liquidity, ASSET_X, ASSET_Y);
				assert_ok!(Dex::provide_liquidity(
					RuntimeOrigin::signed(ALICE),
					provision,
					LP_TOKEN_ID
				));

				// Charlie Provides liquidity of 10k
				mint_asset(CHARLIE, charlie_liquidity + EXISTENTIAL_DEPOSIT, ASSET_X);
				mint_asset(CHARLIE, charlie_liquidity + EXISTENTIAL_DEPOSIT, ASSET_Y);
				let provision = create_asset_amount_pair(charlie_liquidity, ASSET_X, ASSET_Y);
				assert_ok!(Dex::provide_liquidity(
					RuntimeOrigin::signed(CHARLIE),
					provision,
					LP_TOKEN_ID
				));

				// Bob Swaps 10k
				mint_asset(BOB, give + EXISTENTIAL_DEPOSIT, ASSET_X);
				let give_amount = AssetAmount::<Test>::new(ASSET_X, give);
				let expected_taken_amount =
					calculate_expected_taken_amount(give_amount.balance, reserve_x, reserve_y);
				assert_ok!(Dex::swap_limit_take(
					RuntimeOrigin::signed(BOB),
					give_amount,
					expected_taken_amount,
					X_Y_ID
				));

				// Check bob x tokens have been sent and y tokens received
				assert_account_has(BOB, ASSET_X, EXISTENTIAL_DEPOSIT);
				assert_account_has(BOB, ASSET_Y, EXISTENTIAL_DEPOSIT + expected_taken_amount - 1); // one lost to precision

				// Check dex x tokens have been received and y tokens sent
				assert_account_has(
					Dex::dex_account_id(),
					ASSET_X,
					EXISTENTIAL_DEPOSIT + alice_liquidity + charlie_liquidity + give_amount.balance
						- 1, // one lost to precision
				);
				assert_account_has(
					Dex::dex_account_id(),
					ASSET_Y,
					EXISTENTIAL_DEPOSIT + alice_liquidity + charlie_liquidity
						- expected_taken_amount - 1, // one lost to precision
				);

				// Bob gets asset price
				let (expected_x_reserve, expected_y_reserve) = (10_020_000u128, 10_000_110u128);
				assert_ok!(Dex::get_asset_price(RuntimeOrigin::signed(BOB), X_Y_ID, ASSET_X));
				System::assert_last_event(
					AssetPrice {
						price: AssetBalancePairToRatioConverter::convert((
							expected_x_reserve,
							expected_y_reserve,
						)),
					}
					.into(),
				);

				// Alice gets pool and check reserves have changed
				assert_eq!(
					Pools::<Test>::get(&X_Y_ID).expect("pool should exist"),
					create_pool(
						X_Y_ID,
						expected_x_reserve,
						expected_y_reserve,
						alice_liquidity + charlie_liquidity,
					)
				);

				// Check alice lp_tokens have stayed the same
				assert_account_has(ALICE, LP_TOKEN_ID, alice_liquidity);
			});
		}
	}

	fn create_asset_amount_pair(
		of: u128,
		asset_x_id: u32,
		asset_y_id: u32,
	) -> AssetAmountPair<Test> {
		AssetAmountPair {
			amount_x: AssetAmount { asset_id: asset_x_id, balance: of },
			amount_y: AssetAmount { asset_id: asset_y_id, balance: of },
		}
	}

	fn create_bad_asset_amount_pair(
		of: u128,
		asset_x_id: u32,
		asset_y_id: u32,
	) -> AssetAmountPair<Test> {
		AssetAmountPair {
			amount_x: AssetAmount { asset_id: asset_x_id, balance: of - 1 },
			amount_y: AssetAmount { asset_id: asset_y_id, balance: of },
		}
	}

	fn create_asset(asset: u32) {
		assert_ok!(pallet_assets::Pallet::<Test>::create(
			RuntimeOrigin::signed(ADMIN),
			Compact(asset),
			ADMIN,
			EXISTENTIAL_DEPOSIT
		));
	}

	fn create_empty_pool(asset_x_id: u32, asset_y_id: u32) {
		let id_pair: AssetIdPair<Test> =
			AssetIdPair::new(asset_x_id, asset_y_id).expect("id pair should be valid");
		Pools::insert(id_pair.clone(), LiquidityPool::empty_from_pair(id_pair, LP_TOKEN_ID));
	}

	fn create_balanced_pool(id_pair: AssetIdPair<Test>, balance: u128, liquidity: u128) {
		create_pool(id_pair, balance, balance, liquidity);
	}

	fn create_pool(
		id_pair: AssetIdPair<Test>,
		balance_x: u128,
		balance_y: u128,
		liquidity: u128,
	) -> LiquidityPool<Test> {
		let id_pair: AssetIdPair<Test> = AssetIdPair::new(id_pair.asset_x_id, id_pair.asset_y_id)
			.expect("id pair should be valid");
		let mut pool = LiquidityPool::empty_from_pair(id_pair.clone(), LP_TOKEN_ID);
		let mut provision = AssetAmountPair::empty(id_pair.clone());
		provision.amount_x.balance = balance_x;
		provision.amount_y.balance = balance_y;
		pool.asset_amounts = provision;
		pool.total_liquidity = liquidity;
		Pools::insert(id_pair.clone(), pool.clone());
		assert!(Pools::get(&id_pair).is_some());
		pool
	}

	fn mint_asset(recipient: u64, amount: u128, asset: u32) {
		assert_ok!(pallet_assets::Pallet::<Test>::mint(
			RuntimeOrigin::signed(ADMIN),
			Compact(asset),
			recipient,
			amount
		));
		assert_account_has(recipient, asset, amount);
	}

	fn assert_account_has(account_id: u64, asset: u32, expected: u128) {
		let found = pallet_assets::Pallet::<Test>::balance(asset, account_id);
		assert_eq!(
			found, expected,
			"Balance of account {} should be {} but was {}",
			account_id, expected, found
		);
	}

	fn get_account_balance(account_id: u64, asset: u32) -> u128 {
		pallet_assets::Pallet::<Test>::balance(asset, account_id)
	}

	fn calculate_expected_taken_amount(give: u128, reserve_x: u128, reserve_y: u128) -> u128 {
		let fee_pct = <Test as Config>::FeePct::get(); // Swap fee percentage
		let amount_in_with_fee =
			give.ensure_sub(fee_pct * give).expect("Bad taken amount calculation");
		let numerator =
			reserve_y.ensure_mul(amount_in_with_fee).expect("Bad taken amount calculation");
		let denominator = reserve_x
			.ensure_add(amount_in_with_fee.clone())
			.expect("Bad taken amount calculation");
		let take = numerator.ensure_div(denominator).expect("Bad taken amount calculation");

		take
	}

	fn calculate_expected_give_amount(take: u128, reserve_x: u128, reserve_y: u128) -> u128 {
		let fee_pct = <Test as Config>::FeePct::get(); // Swap fee percentage
		let new_reserve_y = reserve_y.ensure_add(take).expect("Bad give amount calculation");
		let new_reserve_x = reserve_x
			.ensure_mul(new_reserve_y)
			.expect("Bad give amount calculation")
			.ensure_div(reserve_y)
			.expect("Bad give amount calculation");
		let raw_give_amount_x =
			new_reserve_x.ensure_sub(reserve_x).expect("Bad give amount calculation");
		let give = raw_give_amount_x
			.ensure_add(fee_pct * raw_give_amount_x)
			.expect("Bad give amount calculation");

		give
	}

	fn init_test_ext() -> TestExternalities {
		new_test_ext(vec![
			(Dex::dex_account_id(), TEN_B),
			(ADMIN, TEN_B),
			(ALICE, TEN_B),
			(BOB, TEN_B),
			(CHARLIE, TEN_B),
		])
	}
}
