/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

use support::{decl_module, decl_storage, decl_event, StorageValue, StorageMap, dispatch::Result, Parameter, ensure};
use runtime_primitives::traits::{CheckedAdd, CheckedMul, As};
use system::ensure_signed;

pub trait Trait: cennzx_spot::Trait {
	type Item: Parameter;
	type ItemId: Parameter + CheckedAdd + Default + From<u8>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type BalanceOf<T> = <T as generic_asset::Trait>::Balance;
pub type AssetIdOf<T> = <T as generic_asset::Trait>::AssetId;
pub type PriceOf<T> = (AssetIdOf<T>, BalanceOf<T>);


// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as XPay {
		pub Items get(item): map T::ItemId => Option<T::Item>;
		pub ItemOwners get(item_owner): map T::ItemId => Option<T::AccountId>;
		pub ItemQuantities get(item_quantity): map T::ItemId => u32;
		pub ItemPrices get(item_price): map T::ItemId => Option<PriceOf<T>>;
		
		pub NextItemId get(next_item_id): T::ItemId;
	}
}

decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    // Use default implementation of deposit_event
    fn deposit_event<T>() = default;

	pub fn create_item(origin, quantity: u32, item: T::Item, price_asset_id: AssetIdOf<T>, price_amount: BalanceOf<T>) -> Result {
		// Ensure this is from user transaction
		let origin = ensure_signed(origin)?;

		// Call a getter to access storage
		let item_id = Self::next_item_id();

		// The last available id serves as the overflow mark and won't be used.
		// Use checked_add to avoid overflow
		// Use ? operator to early exit on error
		let next_item_id = item_id.checked_add(&1.into()).ok_or_else(||"No new item id is available.")?;

		// Update stroage value
		<NextItemId<T>>::put(next_item_id);
		
		let price = (price_asset_id, price_amount);
			
		// Update storage map
		<Items<T>>::insert(item_id.clone(), item.clone());
		<ItemOwners<T>>::insert(item_id.clone(), origin.clone());
		<ItemQuantities<T>>::insert(item_id.clone(), quantity);
		<ItemPrices<T>>::insert(item_id.clone(), price.clone());

		// Emit an on-chain event
		Self::deposit_event(RawEvent::ItemCreated(origin, item_id, quantity, item, price));

		// Indicates method executed successfully
		Ok(())
	}

	pub fn add_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
		// Ensure this is from user transaction
		let origin = ensure_signed(origin)?;

		// Modify storage
		// Use saturating_add to avoid overflow
		<ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_add(quantity));

		// Emit an on-chain event
		Self::deposit_event(RawEvent::ItemAdded(origin, item_id.clone(), Self::item_quantity(item_id)));

		// Indicates method executed successfully
		Ok(())
	}

  pub fn remove_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
	// Ensure this is from user transaction
  let origin = ensure_signed(origin)?;

  // Modify storage
  // Use saturating_sub to avoid underflow
  <ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_sub(quantity));

  // Emit an on-chain event
  Self::deposit_event(RawEvent::ItemRemoved(origin, item_id.clone(), Self::item_quantity(item_id)));

  // Indicates method executed successfully
  Ok(())
}
pub fn update_item(origin, item_id: T::ItemId, quantity: u32, price_asset_id: AssetIdOf<T>, price_amount: BalanceOf<T>) -> Result {
    // Ensure this is from user transaction
    let origin = ensure_signed(origin)?;

    // ensure macro enforces precondition before continuting
    ensure!(<Items<T>>::exists(item_id.clone()), "Item did not exist");

    // Update item quantity
    <ItemQuantities<T>>::insert(item_id.clone(), quantity);
    
    // Update item price
    let price = (price_asset_id, price_amount);
    <ItemPrices<T>>::insert(item_id.clone(), price.clone());

    // Emit an on-chain event
    Self::deposit_event(RawEvent::ItemUpdated(origin, item_id, quantity, price));

    // Indicates method executed successfully
    Ok(())
}
pub fn purchase_item(origin, quantity: u32, item_id: T::ItemId, paying_asset_id: AssetIdOf<T>, max_total_paying_amount: BalanceOf<T>) -> Result {
	// Ensure this is from user transaction
  let origin = ensure_signed(origin)?;

  // Calcualte the new quantity after transaction
  // Use checked_sub to ensure no underflow, which means user is trying to buy too many items
  let new_quantity = Self::item_quantity(item_id.clone()).checked_sub(quantity).ok_or_else(||"Not enough quantity")?;
  let item_price = Self::item_price(item_id.clone()).ok_or_else(||"No item price")?;
  let seller = Self::item_owner(item_id.clone()).ok_or_else(||"No item owner")?;

  // Calculate the total amount that merchant required for this tranaction
  // Use checked_mul to ensure no overflow
  let total_price_amount = item_price.1.checked_mul(&As::sa(quantity as u64)).ok_or_else(||"Total price overflow")?;

  // Check if the paying asset is same as the desire receving asset
  if item_price.0 == paying_asset_id {
    // Same asset, GA transfer

    // Ensure user is willing to pay enough of assets
    ensure!(total_price_amount < max_total_paying_amount, "User paying price too low");

    // Make a transfer via GenericAsset module
    // Use of ? operator will trigger exit if the payment failed for any reasons
    <generic_asset::Module<T>>::make_transfer_with_event(&item_price.0, &origin, &seller, total_price_amount)?;
  } else {
    // Different asset, CENNZX-Spot transfer

    // Make a transfer via CENNZ-X Spot module
    // Use of ? operator will trigger exit if the payment failed for any reasons
    <cennzx_spot::Module<T>>::make_asset_swap_output(
      &origin,             	// buyer
      &seller,             	// recipient
      &paying_asset_id,  		// asset_sold
      &item_price.0,       	// asset_bought
      item_price.1,       	// buy_amount
      max_total_paying_amount,  // max_paying_amount
      <cennzx_spot::Module<T>>::fee_rate() // fee_rate
    )?;
  }

  // Update item quanity
  <ItemQuantities<T>>::insert(item_id.clone(), new_quantity);

  // Emit an on-chain event
  Self::deposit_event(RawEvent::ItemSold(origin, item_id, quantity));

  // Indicates method executed successfully
  Ok(())
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Item,
		<T as Trait>::ItemId,
		Price = PriceOf<T>,
	{
		/// New item created. (transactor, item_id, quantity, item, price)
		ItemCreated(AccountId, ItemId, u32, Item, Price),
		/// More items added. (transactor, item_id, new_quantity)
		ItemAdded(AccountId, ItemId, u32),
		/// Items removed. (transactor, item_id, new_quantity)
		ItemRemoved(AccountId, ItemId, u32),
		/// Item updated. (transactor, item_id, new_quantity, new_price)
		ItemUpdated(AccountId, ItemId, u32, Price),
		/// Item sold. (transactor, item_id, quantity)
		ItemSold(AccountId, ItemId, u32),
	}
);

/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}
	type TemplateModule = Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn it_works_for_default_value() {
		with_externalities(&mut new_test_ext(), || {
			// Just a dummy test for the dummy funtion `do_something`
			// calling the `do_something` function with a value 42
			assert_ok!(TemplateModule::do_something(Origin::signed(1), 42));
			// asserting that the stored value is equal to what we stored
			assert_eq!(TemplateModule::something(), Some(42));
		});
	}
}
