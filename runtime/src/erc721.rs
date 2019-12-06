// Port of the OpenZeppelin ERC721 and ERC721Enumerable contracts to Parity Substrate
// https://github.com/OpenZeppelin/openzeppelin-solidity/tree/master/contracts/token/ERC721

use codec::{Encode};
use system::ensure_signed;
use sr_primitives::traits::{Hash};
use rstd::prelude::*;
use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue,
};
use sr_primitives::traits::{ CheckedAdd, CheckedSub};
// use sr_primitives::RuntimeDebug;

// use rstd::{result, cmp};


pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    // type Index: Parameter + Member + Default + Copy + SimpleArithmetic;
}

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash
    {
        Transfer(Option<AccountId>, Option<AccountId>, Hash),
        Approval(AccountId, AccountId, Hash),
        ApprovalForAll(AccountId, AccountId, bool),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as ERC721Storage {
        // Start ERC721 : Storage & Getters //
        // account id => count
        OwnedTokensCount get(balance_of): map T::AccountId => T::Index;
        // token id => account id
        pub TokenOwner get(owner_of): map T::Hash => Option<T::AccountId>;
        // token id => approved account
        TokenApprovals get(get_approved): map T::Hash => Option<T::AccountId>;
        // (token_id, account_id) => bool
        OperatorApprovals get(is_approved_for_all): map (T::AccountId, T::AccountId) => bool;
        // End ERC721 : Storage & Getters //

        // Start ERC721 : Enumerable : Storage & Getters //
        TotalSupply get(total_supply): T::Index;
        AllTokens get(token_by_index): map T::Index => T::Hash;
        AllTokensIndex: map T::Hash => T::Index;
        // (account_id, index) => token_id
        OwnedTokens get(token_of_owner_by_index): map (T::AccountId, T::Index) => T::Hash;
        OwnedTokensIndex get(get_owned_index): map T::Hash => T::Index;
        // Start ERC721 : Enumerable : Storage & Getters //

        // Not a part of the ERC721 specification, but used in random token generation
        Nonce: u64;
        // test debug 
        // TestDebugs get(get_test_debug): Option<TestDebug<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn deposit_event() = default;

        // Start ERC721 : Public Functions //
        fn approve(origin, to: T::AccountId, token_id: T::Hash) -> Result {
            let sender = ensure_signed(origin)?;
            let owner = match Self::owner_of(token_id) {
                Some(c) => c,
                None => return Err("No owner for this token"),
            };

            ensure!(to != owner, "Owner is implicitly approved");
            ensure!(sender == owner || Self::is_approved_for_all((owner.clone(), sender.clone())), "You are not allowed to approve for this token");

            <TokenApprovals<T>>::insert(&token_id, &to);

            Self::deposit_event(RawEvent::Approval(owner, to, token_id));

            Ok(())
        }

        fn set_approval_for_all(origin, to: T::AccountId, approved: bool) -> Result {
            let sender = ensure_signed(origin)?;
            Self::_set_approval_for_all(sender, to, approved)
        }

        // transfer_from will transfer to addresses even without a balance
        fn transfer_from(origin, from: T::AccountId, to: T::AccountId, token_id: T::Hash) -> Result {
            let sender = ensure_signed(origin)?;
            ensure!(Self::_is_approved_or_owner(sender, token_id), "You do not own this token");

            Self::_transfer_from(from, to, token_id)?;

            Ok(())
        }
        // safe_transfer_from checks that the recieving address has enough balance to satisfy the ExistentialDeposit
        // This is not quite what it does on Ethereum, but in the same spirit...
        // fn safe_transfer_from(origin, from: T::AccountId, to: T::AccountId, token_id: T::Hash) -> Result {
            // let to_balance = <balances::Module<T>>::free_balance(&to);
            // ensure!(!to_balance.is_zero(), "'to' account does not satisfy the `ExistentialDeposit` requirement");
            // to do, check from other module's balance

        //     Self::transfer_from(origin, from, to, token_id)?;

        //     Ok(())
        // }

        fn create_token(origin) -> Result {
            let sender = ensure_signed(origin)?;
            let nonce = Nonce::get();
            // let random_hash = (<system::Module<T>>::random_seed(), &sender, nonce).using_encoded(<T as system::Trait>::Hashing::hash);
            let random_hash = (&sender, nonce).using_encoded(<T as system::Trait>::Hashing::hash);
            
            Self::_mint(sender, random_hash)?;
            Nonce::mutate(|n| *n += 1);

            // Ok(random_hash)
            Ok(())
        }
        fn burn_token(origin, token_id: T::Hash) -> Result{
            let sender = ensure_signed(origin)?;
            let owner = Self::owner_of(&token_id);
            if owner.is_none(){
                return Err("token not found")
            };
            ensure!(owner.unwrap() == sender, "not own this token, cannot burn it");
            Self::_burn(token_id)
        }

    }
}

impl<T: Trait> Module<T> {
    // Start ERC721 : Internal Functions //
    pub fn _exists(token_id: T::Hash) -> bool {
        return <TokenOwner<T>>::exists(token_id);
    }
    pub fn _set_approval_for_all(sender:T::AccountId, to: T::AccountId, approved: bool) -> Result{
        ensure!(to != sender, "You are already implicity approved for your own actions");
        <OperatorApprovals<T>>::insert((sender.clone(), to.clone()), approved);

        Self::deposit_event(RawEvent::ApprovalForAll(sender, to, approved));

        Ok(())
    }

    pub fn _is_approved_or_owner(spender: T::AccountId, token_id: T::Hash) -> bool {
        let owner = Self::owner_of(token_id);
        let approved_user = Self::get_approved(token_id);

        let approved_as_owner = match owner {
            Some(ref o) => o == &spender,
            None => false,
        };

        let approved_as_delegate = match owner {
            Some(d) => Self::is_approved_for_all((d, spender.clone())),
            None => false,
        };

        let approved_as_user = match approved_user {
            Some(u) => u == spender,
            None => false,
        };

        return approved_as_owner || approved_as_user || approved_as_delegate
    }

    pub fn _mint(to: T::AccountId, token_id: T::Hash) -> Result {
        ensure!(!Self::_exists(token_id), "Token already exists");
        let balance_of = Self::balance_of(&to);

        let new_balance_of = match balance_of.checked_add(&1.into()) {
            Some(c) => c,
            None => return Err("Overflow adding a new token to account balance"),
        };

        // Writing to storage begins here
        Self::_add_token_to_all_tokens_enumeration(token_id)?;
        Self::_add_token_to_owner_enumeration(to.clone(), token_id)?;

        <TokenOwner<T>>::insert(token_id, &to);
        <OwnedTokensCount<T>>::insert(&to, new_balance_of);

        Self::deposit_event(RawEvent::Transfer(None, Some(to), token_id));

        Ok(())
    }

    pub fn _burn(token_id: T::Hash) -> Result {
        let owner = match Self::owner_of(token_id) {
            Some(c) => c,
            None => return Err("No owner for this token"),
        };

        let balance_of = Self::balance_of(&owner);

        let new_balance_of = match balance_of.checked_sub(&1.into()) {
            Some(c) => c,
            None => return Err("Underflow subtracting a token to account balance"),
        };

        // Writing to storage begins here
        Self::_remove_token_from_all_tokens_enumeration(token_id)?;
        Self::_remove_token_from_owner_enumeration(owner.clone(), token_id)?;
        <OwnedTokensIndex<T>>::remove(token_id);

        Self::_clear_approval(token_id)?;

        <OwnedTokensCount<T>>::insert(&owner, new_balance_of);
        <TokenOwner<T>>::remove(token_id);

        Self::deposit_event(RawEvent::Transfer(Some(owner), None, token_id));

        Ok(())
    }

    pub fn _transfer_from(from: T::AccountId, to: T::AccountId, token_id: T::Hash) -> Result {
        let owner = match Self::owner_of(token_id) {
            Some(c) => c,
            None => return Err("No owner for this token"),
        };

        ensure!(owner == from, "'from' account does not own this token");

        let balance_of_from = Self::balance_of(&from);
        let balance_of_to = Self::balance_of(&to);

        let new_balance_of_from = match balance_of_from.checked_sub(&1.into()) {
            Some (c) => c,
            None => return Err("Transfer causes underflow of 'from' token balance"),
        };

        let new_balance_of_to = match balance_of_to.checked_add(&1.into()) {
            Some(c) => c,
            None => return Err("Transfer causes overflow of 'to' token balance"),
        };

        // Writing to storage begins here
        Self::_remove_token_from_owner_enumeration(from.clone(), token_id)?;
        Self::_add_token_to_owner_enumeration(to.clone(), token_id)?;
        
        Self::_clear_approval(token_id)?;
        <OwnedTokensCount<T>>::insert(&from, new_balance_of_from);
        <OwnedTokensCount<T>>::insert(&to, new_balance_of_to);
        <TokenOwner<T>>::insert(&token_id, &to);

        Self::deposit_event(RawEvent::Transfer(Some(from), Some(to), token_id));
        
        Ok(())
    }

    fn _clear_approval(token_id: T::Hash) -> Result{
        <TokenApprovals<T>>::remove(token_id);

        Ok(())
    }
    // End ERC721 : Internal Functions //

    // Start ERC721 : Enumerable : Internal Functions //
    fn _add_token_to_owner_enumeration(to: T::AccountId, token_id: T::Hash) -> Result {
        let new_token_index = Self::balance_of(&to);

        <OwnedTokensIndex<T>>::insert(token_id, new_token_index);
        <OwnedTokens<T>>::insert((to, new_token_index), token_id);

        Ok(())
    }

    fn _add_token_to_all_tokens_enumeration(token_id: T::Hash) -> Result {
        let total_supply = Self::total_supply();

        // Should never fail since overflow on user balance is checked before this
        let new_total_supply = match total_supply.checked_add(&1.into()) {
            Some (c) => c,
            None => return Err("Overflow when adding new token to total supply"),
        };

        let new_token_index = total_supply;

        <AllTokensIndex<T>>::insert(token_id, new_token_index);
        <AllTokens<T>>::insert(new_token_index, token_id);
        <TotalSupply<T>>::put(new_total_supply);

        Ok(())
    }

    fn _remove_token_from_owner_enumeration(from: T::AccountId, token_id: T::Hash) -> Result {
        let balance_of_from = Self::balance_of(&from);

        // Should never fail because same check happens before this call is made
        let last_token_index = match balance_of_from.checked_sub(&1.into()) {
            Some (c) => c,
            None => return Err("Transfer causes underflow of 'from' token balance"),
        };
        
        let token_index = <OwnedTokensIndex<T>>::get(token_id);

        if token_index != last_token_index {
            let last_token_id = <OwnedTokens<T>>::get((from.clone(), last_token_index));
            <OwnedTokens<T>>::insert((from.clone(), token_index), last_token_id);
            <OwnedTokensIndex<T>>::insert(last_token_id, token_index);
        }

        <OwnedTokens<T>>::remove((from, last_token_index));
        // OpenZeppelin does not do this... should I?
        <OwnedTokensIndex<T>>::remove(token_id);

        Ok(())
    }

    fn _remove_token_from_all_tokens_enumeration(token_id: T::Hash) -> Result {
        let total_supply = Self::total_supply();

        // Should never fail because balance of underflow is checked before this
        let new_total_supply = match total_supply.checked_sub(&1.into()) {
            Some(c) => c,
            None => return Err("Underflow removing token from total supply"),
        };

        let last_token_index = new_total_supply;

        let token_index = <AllTokensIndex<T>>::get(token_id);

        let last_token_id = <AllTokens<T>>::get(last_token_index);

        <AllTokens<T>>::insert(token_index, last_token_id);
        <AllTokensIndex<T>>::insert(last_token_id, token_index);

        <AllTokens<T>>::remove(last_token_index);
        <AllTokensIndex<T>>::remove(token_id);

        <TotalSupply<T>>::put(new_total_supply);

        Ok(())
    }
    // End ERC721 : Enumerable : Internal Functions //
}

// test cases

#[cfg(test)]
mod tests {
        use super::*;

        use primitives::{H256};
        use support::{impl_outer_origin, parameter_types};
        use sr_primitives::{traits::{BlakeTwo256, IdentityLookup}, testing::Header};
        use sr_primitives::weights::Weight;
        use sr_primitives::Perbill;

        impl_outer_origin! {
            pub enum Origin for Test {}
        }
        // For testing the module, we construct most of a mock runtime. This means
        // first constructing a configuration type (`Test`) which `impl`s each of the
        // configuration traits of modules we want to use.
       #[derive(Clone, Eq, PartialEq, Debug)]
        pub struct Test;
        parameter_types! {
            pub const BlockHashCount: u64 = 250;
            pub const MaximumBlockWeight: Weight = 1024;
            pub const MaximumBlockLength: u32 = 2 * 1024;
            pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
        }
        impl system::Trait for Test {
                type Origin = Origin;
                type Call = ();
                type Index = u64;
                type BlockNumber = u64;
                type Hash = H256;
                type Hashing = BlakeTwo256;
                type AccountId = u64;
                type Lookup = IdentityLookup<Self::AccountId>;
                type Header = Header;
                type Event = ();
                type BlockHashCount = BlockHashCount;
                type MaximumBlockWeight = MaximumBlockWeight;
                type MaximumBlockLength = MaximumBlockLength;
                type AvailableBlockRatio = AvailableBlockRatio;
                type Version = ();
        }
        impl Trait for Test {
            type Event = ();
        }


        // parameter_types! {
        //         pub const ExistentialDeposit: u64 = 0;
        //         pub const TransferFee: u64 = 0;
        //         pub const CreationFee: u64 = 0;
        // }
        // impl balances::Trait for Test {
        //     type Balance = u64;
        //     type OnFreeBalanceZero = ();
        //     type OnNewAccount = ();
        //     type Event = ();
        //     type DustRemoval = ();
        //     type TransferPayment = ();
        //     type ExistentialDeposit = ();
        //     type TransferFee = TransferFee;
        //     type CreationFee = CreationFee;
        // }

        // This function basically just builds a genesis storage key/value store according to
        // our desired mockup.
        fn new_test_ext() -> runtime_io::TestExternalities {
            system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
        }
        pub type Erc721Module = Module<Test>;

        #[test]
        fn create_token_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let alice = 1;
                Erc721Module::create_token(Origin::signed(bob));
                Erc721Module::create_token(Origin::signed(alice));
                Erc721Module::create_token(Origin::signed(alice));

                let tk_0 = Erc721Module::token_by_index(0);
                let tk_1 = Erc721Module::token_by_index(1);
                let tk_2 = Erc721Module::token_by_index(2);

                let tk_bob_0 = Erc721Module::token_of_owner_by_index((bob, 0));
                let tk_alice_0 = Erc721Module::token_of_owner_by_index((alice, 0));
                let tk_alice_1 = Erc721Module::token_of_owner_by_index((alice, 1));

                assert_eq!(tk_0, tk_bob_0);
                assert_eq!(tk_1, tk_alice_0);
                assert_eq!(tk_2, tk_alice_1);

            });
        }

        #[test]
        fn burn_token_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let alice = 1;
                Erc721Module::create_token(Origin::signed(bob));

                let token_id = Erc721Module::token_by_index(0);
                println!("{}",token_id);
                let res = Erc721Module::burn_token(Origin::signed(bob),token_id);
                let token_id = Erc721Module::token_by_index(0);
                println!("{}",token_id);
                
                // assert_eq!(res, Ok(()) );


            });
        }


        #[test]
        fn approve_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let alice = 1;
                Erc721Module::create_token(Origin::signed(bob));
                Erc721Module::create_token(Origin::signed(alice));
                
                let tk_0 = Erc721Module::token_by_index(0);
                let tk_1 = Erc721Module::token_by_index(1);

                let approval_account = Erc721Module::get_approved(tk_1);
                assert_eq!(approval_account, None);

                Erc721Module::approve(Origin::signed(alice), bob, tk_1);
                let approval_account = Erc721Module::get_approved(tk_1).unwrap();
                
                assert_eq!(approval_account, bob);

            });
        }
        

        #[test]
        fn approve_all_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let alice = 1;
                Erc721Module::create_token(Origin::signed(bob));
                Erc721Module::create_token(Origin::signed(alice));
                
                let tk_0 = Erc721Module::token_by_index(0);
                let tk_1 = Erc721Module::token_by_index(1);

                let bool_is_approved_for_all = Erc721Module::is_approved_for_all((alice, bob));
                assert_eq!(bool_is_approved_for_all, false);

                Erc721Module::set_approval_for_all(Origin::signed(alice), bob, true);
                let bool_is_approved_for_all = Erc721Module::is_approved_for_all((alice, bob));
                assert_eq!(bool_is_approved_for_all, true);

                Erc721Module::set_approval_for_all(Origin::signed(alice), bob, false);
                let bool_is_approved_for_all = Erc721Module::is_approved_for_all((alice, bob));
                assert_eq!(bool_is_approved_for_all, false);

            });
        }

        #[test]
        fn transfer_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let alice = 1;
                Erc721Module::create_token(Origin::signed(bob));
                Erc721Module::create_token(Origin::signed(alice));
                
                let tk_0 = Erc721Module::token_by_index(0);
                let tk_1 = Erc721Module::token_by_index(1);

                assert_eq!(Erc721Module::get_owned_index(tk_1), 0);
                // transfer without approval
                let res = Erc721Module::transfer_from(Origin::signed(bob), alice, bob, tk_1);
                assert_eq!(res, Err("You do not own this token"));
                // transfer after approve one token
                Erc721Module::approve(Origin::signed(alice), bob, tk_1);
                let res = Erc721Module::transfer_from(Origin::signed(bob), alice,  bob, tk_1);
                // let res = Erc721Module::transfer_from(Origin::signed(alice), bob, tk_1);
                assert_eq!(res, Ok(()));
                assert_eq!(Erc721Module::owner_of(tk_1).unwrap(), bob);
                assert_eq!(Erc721Module::get_owned_index(tk_1), 1);

                // reset 
                Erc721Module::approve(Origin::signed(bob), alice, tk_1);
                assert_eq!(Erc721Module::_is_approved_or_owner(alice, tk_1), true);
                assert_eq!(Erc721Module::_is_approved_or_owner(bob, tk_1), true);
                Erc721Module::transfer_from(Origin::signed(alice), bob,  alice, tk_1);
                assert_eq!(Erc721Module::_is_approved_or_owner(bob, tk_1), false);
                // transfer after approve all to an account
                Erc721Module::set_approval_for_all(Origin::signed(alice), bob, true);
                let res = Erc721Module::transfer_from(Origin::signed(alice), alice, bob, tk_1);
                assert_eq!(res, Ok(()));
                assert_eq!(Erc721Module::owner_of(tk_1).unwrap(), bob);
                assert_eq!(Erc721Module::get_owned_index(tk_1), 1);

            });
        }
        
}