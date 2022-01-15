#![cfg_attr(not(feature = "std"), no_std)]
include!("./github.rs");
include!("./dot.rs");

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	/// 声明引用
	use core::{convert::TryInto};
	use parity_scale_codec::{Decode, Encode};
	use frame_support::pallet_prelude::*;
	use frame_system::{
		pallet_prelude::*,
		offchain::{
			AppCrypto, CreateSignedTransaction, SendSignedTransaction, SendUnsignedTransaction,
			SignedPayload, Signer, SigningTypes, SubmitTransaction,
		},
	};
	use sp_core::{crypto::KeyTypeId};
	use sp_arithmetic::per_things::Permill;
	use sp_runtime::{
		offchain as rt_offchain,
		traits::{
			BlockNumberProvider
		},
		offchain::{
			storage::StorageValueRef,
			storage_lock::{BlockAndTime, StorageLock},
		},
		transaction_validity::{
			InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
		},
		RuntimeDebug,
	};
	use sp_std::{collections::vec_deque::VecDeque, prelude::*, str};
	use serde::{Deserialize};
	

	/// 定义常量
	pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"demo");
	const NUM_VEC_LEN: usize = 10;
	const UNSIGNED_TXS_PRIORITY: u64 = 100;
	const LOCK_BLOCK_EXPIRATION: u32 = 3;
	const LOCK_TIMEOUT_EXPIRATION: u64 = 4000;
	
	pub mod crypto {
		use crate::KEY_TYPE;
		use sp_core::sr25519::Signature as Sr25519Signature;
		use sp_runtime::app_crypto::{app_crypto, sr25519};
		use sp_runtime::{traits::Verify, MultiSignature, MultiSigner};

		app_crypto!(sr25519, KEY_TYPE);

		pub struct TestAuthId;

		impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
			type RuntimeAppPublic = Public;
			type GenericSignature = sp_core::sr25519::Signature;
			type GenericPublic = sp_core::sr25519::Public;
		}

		impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature> for TestAuthId {
			type RuntimeAppPublic = Public;
			type GenericSignature = sp_core::sr25519::Signature;
			type GenericPublic = sp_core::sr25519::Public;
		}
	}


	/// 定义结构体
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct Payload<Public> {
		number: u64,
		public: Public,
	}

	impl<T: SigningTypes> SignedPayload<T> for Payload<T::Public> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	#[derive(Debug, Deserialize, Encode, Decode, Default)]
	struct IndexingData(Vec<u8>, u64);

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct PricePayload<Public, BlockNumber> {
		block_number: BlockNumber,
		public: Public,
		price: (u64, Permill),
	}

	impl<T: SigningTypes> SignedPayload<T> for PricePayload<T::Public, T::BlockNumber> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	
	/// 定义pallet
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);	


	/// 定义config
	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Call: From<Call<Self>>;
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
	}


    /// 定义存储
	#[pallet::storage]
	#[pallet::getter(fn numbers)]
	pub type Numbers<T> = StorageValue<_, VecDeque<u64>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn prices)]
	pub type Prices<T> = StorageValue<_, VecDeque<(u64, Permill)>, ValueQuery>;


	/// 定义事件
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewNumber(Option<T::AccountId>, u64),
		NewPrice(Option<T::AccountId>, (u64, Permill)),
	}

	/// 定义error
	#[pallet::error]
	pub enum Error<T> {
		UnknownOffchainMux,
		NoLocalAcctForSigning,
		OffchainSignedTxError,
		OffchainUnsignedTxError,
		OffchainUnsignedTxSignedPayloadError,
		HttpFetchingError,
		ConvertError
	}

	/// 定义 pallet hooks
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		
		/// 定义链下工作机
		fn offchain_worker(block_number: T::BlockNumber) {
			log::info!("Hello World from offchain workers!");

			const TX_TYPES: u32 = 5;
			let modu = block_number.try_into().map_or(TX_TYPES, |bn: usize| (bn as u32) % TX_TYPES);
			let result = match modu {
				0 => Self::offchain_signed_tx(block_number),
				1 => Self::offchain_unsigned_tx(block_number),
				2 => Self::offchain_unsigned_tx_signed_payload(block_number),
				3 => Self::fetch_github_info(),
				4 => Self::fetch_dot_price_info(block_number),
				_ => Err(Error::<T>::UnknownOffchainMux),
			};

			if let Err(e) = result {
				log::error!("offchain_worker error: {:?}", e);
			}
		}
	}

	/// 定义pallet call
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		pub fn submit_number_signed(origin: OriginFor<T>, number: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			log::info!("submit_number_signed: ({}, {:?})", number, who);
			Self::append_or_replace_number(number);
			Self::deposit_event(Event::NewNumber(Some(who), number));
			Ok(())
		}

		#[pallet::weight(10000)]
		pub fn submit_number_unsigned(origin: OriginFor<T>, number: u64) -> DispatchResult {
			let _ = ensure_none(origin)?;
			log::info!("submit_number_unsigned: {}", number);
			Self::append_or_replace_number(number);
			Self::deposit_event(Event::NewNumber(None, number));
			Ok(())
		}

		#[pallet::weight(10000)]
		pub fn submit_number_unsigned_with_signed_payload(origin: OriginFor<T>, payload: Payload<T::Public>, _signature: T::Signature) -> DispatchResult {
			let _ = ensure_none(origin)?;
			let Payload { number, public } = payload;
			log::info!("submit_number_unsigned_with_signed_payload: ({}, {:?})", number, public);
			Self::append_or_replace_number(number);
			Self::deposit_event(Event::NewNumber(None, number));
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn submit_price_unsigned_with_signed_payload(origin: OriginFor<T>,price_payload: PricePayload<T::Public, T::BlockNumber>,_signature: T::Signature) -> DispatchResult {
			let _ = ensure_none(origin)?;
			let PricePayload { block_number: _, public: _, price } = price_payload;
			log::info!("submit_price_unsigned_with_signed_payload");
			Self::append_or_replace_price(price);
			Self::deposit_event(Event::NewPrice(None, price));
			Ok(())
		}
	}

	/// 为pallet实现无需验签方法
	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let valid_tx = |provide| ValidTransaction::with_tag_prefix("ocw-demo")
			.priority(UNSIGNED_TXS_PRIORITY)
			.and_provides([&provide])
			.longevity(3)
			.propagate(true)
			.build();

			match call {
				Call::submit_number_unsigned(_number) => valid_tx(b"submit_number_unsigned".to_vec()),
				Call::submit_number_unsigned_with_signed_payload(ref payload, ref signature) => {
					if !SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone()) {
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(b"submit_number_unsigned_with_signed_payload".to_vec())
				},
				Call::submit_price_unsigned_with_signed_payload(ref price_payload,ref signature) => {
					if !SignedPayload::<T>::verify::<T::AuthorityId>(price_payload, signature.clone()) {
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(b"submit_price_unsigned_with_signed_payload".to_vec())
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	impl<T: Config> Pallet<T> {

		fn fetch_github_info() -> Result<(), Error<T>> {
			log::info!("fetch github info ...");
			let s_info = StorageValueRef::persistent(b"offchain-demo::gh-info");
			if let Ok(Some(gh_info)) = s_info.get::<super::github::GithubInfo>() {
				log::info!("cached github-info: {:?}", gh_info);
				return Ok(());
			}
			let mut lock = StorageLock::<BlockAndTime<Self>>::with_block_and_time_deadline(b"offchain-demo::lock",
			 	LOCK_BLOCK_EXPIRATION, rt_offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION));

			if let Ok(_guard) = lock.try_lock() {
				match super::github::fetch_n_parse() {
					Ok(gh_info) => { s_info.set(&gh_info); }
					Err(_) => { return Err(Error::<T>::HttpFetchingError);}
				}
			}
			Ok(())
		}

		/// 通过offchain worker发送http请求获取dot当前价格并通过无签名交易上链条
		/// 使用无签名交易但是需要验证签名原因:
		/// 1) dot为公开数据,与账户无关,且通过offchain worker完成请求,链上无需多余计算,数据在链上存储空间也固定不需要每次都开辟，固也不需要开辟多余存储空间
		/// 2) 但是为了验证数据的可靠性还是需要数据签名验证
		fn fetch_dot_price_info(block_number: T::BlockNumber) -> Result<(), Error<T>> {
			log::info!("fetch dot price info ...");

			// 获取dot价格
			let price_usd = match super::dot::fetch_dot_price_parse() {
				Ok(price_info) => { 
					log::info!("price_info: {:?}", price_info);
					price_info.price_usd
				 }
				Err(_) => { return Err(Error::<T>::HttpFetchingError);}
			};
			log::info!("price_usd: {:?}", price_usd);

			// 转换格式
			let price_usd_split = super::dot::parse_price_usd(price_usd).ok_or_else(|| {Error::<T>::ConvertError})?;
			log::info!("price_usd_split_integer: {:?}, price_usd_split_permil: {:?}", price_usd_split.0 ,price_usd_split.1);

			// 交易上链
			let result = Signer::<T, T::AuthorityId>::any_account().send_unsigned_transaction(
				|account| PricePayload { block_number, public: account.public.clone(), price: price_usd_split},
				Call::submit_price_unsigned_with_signed_payload,
			);

			// 处理结果
			if let Some((_, res)) = result {
				return res.map_err(|_| {<Error<T>>::OffchainUnsignedTxSignedPayloadError});
			} else {
				return Err(<Error<T>>::NoLocalAcctForSigning);
			}
		}

		fn append_or_replace_price(price: (u64, Permill)) {
			Prices::<T>::mutate(|prices| {
				if prices.len() == NUM_VEC_LEN {
					let _ = prices.pop_front();
				}
				prices.push_back(price);
				log::info!("Prices vector: {:?}", prices);
			});
		}
		
		fn append_or_replace_number(number: u64) {
			Numbers::<T>::mutate(|numbers| {
				if numbers.len() == NUM_VEC_LEN {
					let _ = numbers.pop_front();
				}
				numbers.push_back(number);
				log::info!("Number vector: {:?}", numbers);
			});
		}
		

		fn offchain_signed_tx(block_number: T::BlockNumber) -> Result<(), Error<T>> {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			let number: u64 = block_number.try_into().unwrap_or(0);
			let result = signer.send_signed_transaction(|_acct|
				Call::submit_number_signed(number)
				);

			if let Some((acc, res)) = result {
				if res.is_err() {
					log::error!("failure: offchain_signed_tx: tx sent: {:?}", acc.id);
					return Err(<Error<T>>::OffchainSignedTxError);
				}
				
				return Ok(());
			}

			log::error!("No local account available");
			Err(<Error<T>>::NoLocalAcctForSigning)
		}

		fn offchain_unsigned_tx(block_number: T::BlockNumber) -> Result<(), Error<T>> {
			let number: u64 = block_number.try_into().unwrap_or(0);
			let call = Call::submit_number_unsigned(number);

			SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|_| {
				log::error!("Failed in offchain_unsigned_tx");
				<Error<T>>::OffchainUnsignedTxError
			})
		}

		fn offchain_unsigned_tx_signed_payload(block_number: T::BlockNumber) -> Result<(), Error<T>> {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			let number: u64 = block_number.try_into().unwrap_or(0);

			if let Some((_, res)) = signer.send_unsigned_transaction(
				|acct| Payload { number, public: acct.public.clone() },
				Call::submit_number_unsigned_with_signed_payload
				) {
				return res.map_err(|_| {
					log::error!("Failed in offchain_unsigned_tx_signed_payload");
					<Error<T>>::OffchainUnsignedTxSignedPayloadError
				});
			}

			log::error!("No local account available");
			Err(<Error<T>>::NoLocalAcctForSigning)
		}
	}

	impl<T: Config> BlockNumberProvider for Pallet<T> {
		type BlockNumber = T::BlockNumber;

		fn current_block_number() -> Self::BlockNumber {
			<frame_system::Pallet<T>>::block_number()
		}
	}
}
