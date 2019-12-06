// Port of the OpenZeppelin ERC721 and ERC721Enumerable contracts to Parity Substrate
// https://github.com/OpenZeppelin/openzeppelin-solidity/tree/master/contracts/token/ERC721

use codec::{Encode, Decode};
use system::ensure_signed;
use sr_primitives::traits::{Hash};
use rstd::prelude::*;
// use rstd::collections::btree_map::BTreeMap;

use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue,
    traits::{
        LockableCurrency, Currency,
    }

};
use sr_primitives::traits::{CheckedAdd, CheckedSub};
use crate::erc721;

// #[cfg(feature = "std")]
// use std::fmt;

pub trait Trait: erc721::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
    
}

pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
// pub type HashOf<T> = <T as Trait>::Hash;
// pub type String = Vec<u8>;

// type NegativeImbalanceOf<T> =
// 	<<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::NegativeImbalance;

#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub enum PermissionType{
    Black = 0,
    White,
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Permission<AccountId> 
    // where AccountId: Member,
    where AccountId: core::fmt::Debug
{
    perm_type: PermissionType,
    // target: TargetType::Asset,
    account: AccountId,
}
impl<AccountId> core::fmt::Display for Permission<AccountId> 
    // where AccountId: Member,
    where AccountId: core::fmt::Debug
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
// #[cfg_attr(feature = "std", derive(Debug))]
pub struct NonfungibleOption<AccountId, Balance> 
    where AccountId: core::fmt::Debug
{
    permissions: Vec<Permission<AccountId>>,
    max_supply: Balance,
    description:Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq,Debug)]
pub struct NftMeta<T> where
    T: Trait
{
    total_supply: BalanceOf<T>, // amount of tokens issued
    issuer: T::AccountId,
    symbol:Vec<u8>,// symbol name of this nft 
    nft_id: T::Hash,
    option: NonfungibleOption<T::AccountId, BalanceOf<T>>,
}

impl<T:Trait> NftMeta<T>  {
    fn reset_permission(&self, sender: T::AccountId) -> Result{
        ensure!(self.issuer == sender, "not authorized to reset permission as not the issuer"); 
        let opt = &self.option;
        let permissions = &opt.permissions;
        for it in permissions.iter(){// it is Permission
            match it.perm_type{
                PermissionType::Black =>{
                    // <T as erc721::Trait>::_set_approval_for_all(sender.clone(), it.account, false);
                    <NftPermissions<T>>::insert((self.nft_id, it.account.clone()), false);
                },
                PermissionType::White =>{
                    // <T as erc721::Trait>::_set_approval_for_all(sender.clone(), it.account, true);
                    <NftPermissions<T>>::insert((self.nft_id, it.account.clone()), true);
                },
            }
        };
        Ok(())
        
    }
    fn validate(&self, account:T::AccountId) -> bool{
        let nft_id = self.nft_id;
        match <NftPermissions<T>>::get((nft_id, account)){
            Some(t) => return t,
            None => return true,
        }
    }
}

#[derive(Encode, Decode, Clone, Default, PartialEq,Debug)]
pub struct Token<T> where
    T: Trait
{
    token_id: T::Hash,
    symbol: Vec<u8>, // symbol of this token
    pub nft_id: T::Hash, // nft_id
    // attributes: BTreeMap<Vec<u8>, Vec<u8>>,
    // attributes: BTreeMap<u8, u64>,
}




decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash
    {
        NonfungibleCreate(AccountId, Hash), 
        NonfungibleUpdate(AccountId, Hash), 
        TokenDestroy(AccountId, Hash), 
        TokenIssue(AccountId, Hash, Hash),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as NFTStorage {
        OwnedNFTsCounter get(nft_counter_owner): map T::AccountId =>  T::Index;
        // map owner accountid -> a vec of nft ids
        OwnedNFTsVec get(get_nfts_owner_vec) : map T::AccountId => Vec<T::Hash>;
        // map index -> nft_id
        // map nft id -> nft meta
        NFTs get(get_nft): map T::Hash => Option<NftMeta<T>>;
        // nft permission control, (nft_id, account) => bool, assert account is not owner
        NftPermissions get(nft_perm): map (T::Hash, T::AccountId) => Option<bool>;
        // map token id -> token
        Tokens get(get_token) : map T::Hash => Option<Token<T>>;
        // map nft id -> vec of token ids
        
        TokensUnderNFTVec get(get_tokens_nft_vec) : map T::Hash => Vec<T::Hash>;
        // map nft_id -> tokens number under this nft
        TokenUnderNftCounter get(get_nft_token_counter) : map T::Hash  => T::Index;


        TotalNFTSupply get(total_nft_supply): T::Index;
        //global index -> nft_id
        AllNFTsIndex get(get_nft_by_index): map  T::Index => T::Hash;
        // map (account, nft under account index) -> nft id
        OwnedNFTs get(nft_of_owner_by_index): map (T::AccountId, T::Index) => T::Hash;
        // map nft id -> nft under account index
        // OwnedNFTsIndex: map T::Hash => T::Index;
        // reserve token, token id -> bool
        ReserveTokens get(get_token_reserve): map T::Hash => bool;

        Nonce: u64;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        // Public Functions //
        pub fn nonfungible_create(origin, symbol : Vec<u8>, max_supply: BalanceOf<T>) -> Result {
            let sender = ensure_signed(origin)?;
            
            Self::_nonfungible_create(sender, &symbol, max_supply)
            // Ok(())
            
        }

        pub fn nonfungible_update(origin, new_issuer: Option<T::AccountId>, new_option: Option<NonfungibleOption<T::AccountId, BalanceOf<T>>>, nft_id: T::Hash) -> Result {
            let sender = ensure_signed(origin)?;
            
            Self::_nonfungible_update(sender, new_issuer, new_option, nft_id)
            // Ok(())
        }
        
        // Not part of ERC721, but allows you to play with the runtime
        pub fn issue_token(origin, nft_id: T::Hash, symbol: Vec<u8>) -> Result {
            let sender = ensure_signed(origin)?;
            Self::_issue_token(sender, nft_id, symbol)
            // Ok(())
        }
        fn destroy_token(origin, token_id: T::Hash) -> Result{
            let sender = ensure_signed(origin)?;
            Self::_destroy_token(sender, token_id)
            // Ok(())
        }
        fn token_reserve(origin, token_id: T::Hash) -> Result{
            let sender = ensure_signed(origin)?;
            Self::_token_reserve(sender, token_id)
        }
        fn token_unreserve(origin, token_id: T::Hash )->Result{
            let sender = ensure_signed(origin)?;
            Self::_token_unreserve(sender, token_id)
        }
    }
}
// 

impl<T: Trait> Module<T> //where <T as system::Trait>::Origin: core::convert::From<<T as system::Trait>::AccountId>
{
    // Start ERC721 : Internal Functions //
    fn _exists(nft_id: T::Hash) -> bool {
        return <NFTs<T>>::exists(nft_id);
    }
    fn _nft_owner_check(owner:T::AccountId, nft_id: T::Hash) -> bool{
        let nft = Self::get_nft(&nft_id).unwrap();
        return owner == nft.issuer
    }

    fn _nonfungible_create(issuer:T::AccountId , symbol : &Vec<u8>, max_supply: BalanceOf<T>) -> Result {
        let option = NonfungibleOption{
            permissions: Vec::<Permission<T::AccountId>>::new(),
            // max_supply : <BalanceOf<T>>::max_value(),
            max_supply : max_supply,
            description : Vec::<u8>::new(),
        };
        let total_nft_count =  Self::total_nft_supply();
        // let nonce = Nonce::get();
        // Nonce::mutate(|n| *n += 1);
        let nft_id =  ( &issuer, total_nft_count).using_encoded(<T as system::Trait>::Hashing::hash);
        ensure!(!Self::_exists(nft_id), "Nft id conflicts.");
        let new_nft = NftMeta{
            total_supply : 0.into(),
            issuer : issuer.clone(),
            symbol: symbol.to_vec(),
            nft_id: nft_id,
            option :option,   
        };

        <NFTs<T>>::insert(&nft_id, new_nft);
        // <TokenUnderNftCounter<T>>::insert(&nft_id, 0); // come across error
        
        let new_total_nft_count = match total_nft_count.checked_add(&1.into()){
            Some(c) => c,
            None => return Err("Overflow adding a new nft"),
        };
        <TotalNFTSupply<T>>::put(new_total_nft_count);

        <AllNFTsIndex<T>>::insert(&total_nft_count, nft_id);

        let owned_nft_count = Self::nft_counter_owner(&issuer);
        let new_owned_nft_count = match owned_nft_count.checked_add(&1.into()){
            Some(c) => c,
            None => return Err("Overflow adding a new nft to account"),
        };
        <OwnedNFTsCounter<T>>::insert(&issuer, new_owned_nft_count);
        // OwnedNFTsIndex<T>::insert(nft_id, owned_nft_count);
        <OwnedNFTsVec<T>>::mutate(&issuer, |x| x.push(nft_id));

        <OwnedNFTs<T>>::insert((issuer.clone(), owned_nft_count), nft_id);
        
        Self::deposit_event(RawEvent::NonfungibleCreate(issuer, nft_id));
        
        Ok(())
    }
    fn _nonfungible_update(issuer:T::AccountId ,  new_issuer: Option<T::AccountId>, new_option: Option<NonfungibleOption<T::AccountId, BalanceOf<T>>>, nft_id: T::Hash) -> Result {
        let mut nft = Self::get_nft(&nft_id).unwrap();
        let mut will_update : u32 = 0;
        ensure!(Self::_nft_owner_check(issuer.clone(),nft_id), "not authorized as not the issuer of this nft");
        if new_issuer.is_some(){
            let new_issuer = new_issuer.unwrap();
            // set new issuer
            nft.issuer = new_issuer;
            will_update += 2;
        };
        if new_option.is_some(){
            let new_option = new_option.unwrap();
            // set new option
            nft.option = new_option.clone();
            nft.reset_permission(issuer.clone())?;
            will_update += 1;
        };
        ensure!(will_update > 0, "neither of issuer or option will be updated");
        <NFTs<T>>::insert(nft_id, nft);
        Self::deposit_event(RawEvent::NonfungibleUpdate(issuer, nft_id));
        
        Ok(())
    }

    pub fn _token_unreserve(issuer: T::AccountId, token_id: T::Hash )->Result{
        let owner = match  <erc721::Module<T>>::owner_of(&token_id){
            Some(c) => c,
            None => return Err("null token under this token_id")
        };
        ensure!(owner == issuer , "not authrized as not the issuer of this token");
        ensure!(Self::get_token_reserve(token_id) == true, "token already unreserved");
        <ReserveTokens<T>>::insert(token_id, false);
        Ok(())
    }
    pub fn _token_reserve(issuer: T::AccountId, token_id: T::Hash )->Result {
        let owner = match  <erc721::Module<T>>::owner_of(&token_id){
            Some(c) => c,
            None => return Err("null token under this token_id")
        };
        ensure!(owner == issuer , "not authrized as not the issuer of this token");
        ensure!(Self::get_token_reserve(token_id) == false, "token already reserved");
        <ReserveTokens<T>>::insert(token_id, true);
        Ok(())
    }
    fn _supply_decrease(nft_id: T::Hash) -> Result{
        let mut nft = match Self::get_nft(&nft_id){
            Some(t) => t,
            None => return Err("nft not found")
        };
        let new_total_supply = match nft.total_supply.checked_sub(&1.into()){
            None => return Err("Overflow decreasing total_supply on this nft"),
            Some(t) => t,
        };
        nft.total_supply = new_total_supply;
        <NFTs<T>>::insert(nft_id, nft);
        Ok(())
    }
    fn _destroy_token(issuer: T::AccountId, token_id: T::Hash )->Result {
        let owner = match  <erc721::Module<T>>::owner_of(&token_id){
            Some(c) => c,
            None => return Err("null token under this token_id")
        };
        ensure!(owner == issuer , "not authrized as not the issuer of this token");
        let token = match Self::get_token(&token_id){
            Some(c) => c,
            None => return Err("null token under this token_id")
        };
        let nft_id = token.nft_id;
        match Self::_supply_decrease(nft_id){
            Err(e) => return Err(e),
            Ok(()) => {},
        };
        // remove from reserve tokens
        <ReserveTokens<T>>::remove(&token_id);
        <Tokens<T>>::remove(&token_id);
        let mut owner_vec = Self::get_tokens_nft_vec(&nft_id);
        
        // owner_vec.remove_item(&token_id);
        let pos = owner_vec.iter().position(|x| *x == token_id);
        match Some(owner_vec.remove(pos.unwrap())){
            Some(c) => {},
            None => {
                // to do, alert this tokenid not list under this nft id
            }
        };

        Self::deposit_event(RawEvent::TokenDestroy(issuer, token_id));
        // call erc721 module functions
        <erc721::Module<T>>::_burn(token_id)

    }
    fn _supply_increase(nft_id: T::Hash) -> Result{
        let mut nft = match Self::get_nft(&nft_id){
            Some(t) => t,
            None => return Err("nft not found")
        };
        let new_total_supply = match nft.total_supply.checked_add(&1.into()){
            None => return Err("Overflow adding total_supply on this nft"),
            Some(t) => t,
        };
        let max = nft.option.max_supply;
        if max <= new_total_supply {
            return Err("Overflow adding total_supply to max_supply")
        };
        nft.total_supply = new_total_supply;
        <NFTs<T>>::insert(nft_id, nft);
        Ok(())
    }
    fn _issue_token(issuer:T::AccountId, nft_id: T::Hash, symbol: Vec<u8>) -> Result{
        let token_nft_idx = Self::get_nft_token_counter(&nft_id);
        ensure!(Self::_exists(nft_id), "Nft id not exist");
        ensure!(Self::_nft_owner_check(issuer.clone(), nft_id), "dont have the auth to issue your token as you are not the issuer of this nft assigned");
        let new_token_nft_idx = match token_nft_idx.checked_add(&1.into()){
            Some(c) => c,
            None => return Err("Overflow adding a new token to an existing nft"),
        };
        let token_id =  (&issuer, new_token_nft_idx, nft_id).using_encoded(<T as system::Trait>::Hashing::hash);
        ensure!(!<erc721::Module<T>>::_exists(token_id), "token id already exists");
        let new_token = Token{
            token_id: token_id,
            symbol: symbol, // symbol of this token
            nft_id: nft_id, // nft_id
        };
        match Self::_supply_increase(nft_id){
            Err(e) => return Err(e),
            Ok(()) => {},
        };
        <ReserveTokens<T>>::insert(token_id, false);
        <Tokens<T>>::insert(&token_id, new_token);
        <TokenUnderNftCounter<T>>::insert(nft_id, new_token_nft_idx);
        // let mut owner_vec = Self::get_tokens_nft_vec(&nft_id);
        // owner_vec.push(token_id);
        <TokensUnderNFTVec<T>>::mutate(&nft_id, |x| x.push(token_id));
        Self::deposit_event(RawEvent::TokenIssue(issuer.clone(), token_id, nft_id));
        // call erc721 module functions
        <erc721::Module<T>>::_mint(issuer, token_id)

    }
    pub fn _reserve_safe_transfer(from:T::AccountId, to: T::AccountId, token_id:T::Hash) -> Result{
        // get nft_id and check validate
        let token = match Self::get_token(&token_id){
            None => return Err("token not found"),
            Some(t) => t,
        };
        let nft_id = &token.nft_id;
        let _is_authorized_token = match <NftPermissions<T>>::get((nft_id, to.clone()) ){
            None => true,
            Some(t) => t,
        };
        ensure!( _is_authorized_token == true, "nft blacklist contains to-account");
        // check reservation
        ensure!(!Self::get_token_reserve(&token_id), "token reserved, transfer now is forbidden");
        // check ownership
        ensure!(<erc721::Module<T>>::_is_approved_or_owner(from.clone(), token_id), "You do not own this token");
        <erc721::Module<T>>::_transfer_from(from, to, token_id)

    }
    // End ERC721 : Enumerable : Internal Functions //
}

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
        type Balance = u64;
        parameter_types! {
            pub const TransferFee: Balance = 0;
            pub const CreationFee: Balance = 0;
        }
        impl balances::Trait for Test {
            type Balance = Balance;
            type OnFreeBalanceZero = ();
            type OnNewAccount = ();
            type Event = ();
            type TransferPayment = ();
            type DustRemoval = ();
            type ExistentialDeposit = ();
            type TransferFee = TransferFee;
            type CreationFee = CreationFee;
        }


        
        impl erc721::Trait for Test{
            type Event = ();
        }
        impl Trait for Test{
            type Event = ();
            type Currency = balances::Module<Self>;
        }

        // This function basically just builds a genesis storage key/value store according to
        // our desired mockup.
        fn new_test_ext() -> runtime_io::TestExternalities {
            system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
        }

        pub type NftsModule = Module<Test>;
        pub type Erc721Module = erc721::Module<Test>;
        #[test]
        fn nonfungible_create_test() {
            new_test_ext().execute_with(|| {
                // let bob = Origin::signed(0);
                let bob = 0;
                let alice = 1;  
                let res = NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                assert_eq!(res, Ok(()));
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                let nft_id_owner = NftsModule::nft_of_owner_by_index((bob, 0));
                assert_eq!(nft_id, nft_id_owner);
                let nft = NftsModule::get_nft(nft_id);

                let nft_vec = NftsModule::get_nfts_owner_vec(bob);
                assert_eq!(nft_vec.len(), 1);
                assert_eq!(nft_vec[0], nft_id);
                let total_nft_count =  NftsModule::total_nft_supply();
                assert_eq!(total_nft_count, 1);
            });
        }
        use rstd::str;
        #[test]
        fn nonfungible_update_test() {
            new_test_ext().execute_with(|| {
                // let bob = Origin::signed(0);
                fn print_option(opt: NonfungibleOption<<Test as system::Trait>::AccountId, BalanceOf<Test>> ){
                    println!("max_supply...{}", opt.max_supply);
                    let descp_str = str::from_utf8(&opt.description).unwrap();
                    println!("description...{}", descp_str);
                    println!("permissions...");
                    for o in opt.permissions.iter(){
                        println!("permission -> {}", o)
                    }
                };
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                let nft_old = NftsModule::get_nft(&nft_id).unwrap();
                assert_eq!(nft_old.issuer , bob);
                assert_eq!(nft_old.symbol , "doggy".as_bytes().to_vec());
                print_option(nft_old.option);
                let mut permissions = Vec::<Permission<<Test as system::Trait>::AccountId>>::new();
                permissions.push(
                    Permission{
                        perm_type:PermissionType::Black,
                        account:alice,
                    }
                );

                permissions.push(
                    Permission{
                        perm_type:PermissionType::White,
                        account:jack,
                    }
                );
                let new_option = NonfungibleOption{
                    permissions,
                    max_supply : 5,
                    description : "doggy".as_bytes().to_vec(),
                };
                NftsModule::nonfungible_update(Origin::signed(bob), Some(bobby), Some(new_option), nft_id);
                let nft_new =  NftsModule::get_nft(&nft_id).unwrap();
                assert_eq!(nft_new.issuer , bobby);
                print_option(nft_new.option);

                
            });
        }
        #[test]
        fn issue_token_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                let res = NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                assert_eq!(res, Ok(()));
                let token_id = Erc721Module::token_by_index(0);
                let token = NftsModule::get_token(token_id).unwrap();
                assert_eq!(token_id, token.token_id);
                assert_eq!(token.symbol, "token_0".as_bytes().to_vec() );
                assert_eq!(token.nft_id, nft_id);
                assert_eq!(NftsModule::get_nft_token_counter(nft_id), 1);
                assert_eq!(NftsModule::get_tokens_nft_vec(nft_id).len(), 1);
            });
        }
        #[test]
        fn destroy_token_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);
                NftsModule::destroy_token(Origin::signed(bob), token_id);
                let res = NftsModule::get_token(token_id);
                assert_eq!(res, None);
                let res = Erc721Module::token_by_index(0);
                let zero = <Test as system::Trait>::Hash::zero();
                assert_eq!(res, zero);

            });
        }

        #[test]
        fn reserve_token_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);
                assert_eq!(NftsModule::get_token_reserve(token_id), false);

                NftsModule::token_reserve(Origin::signed(bob), token_id);
                assert_eq!(NftsModule::get_token_reserve(token_id), true);

                NftsModule::token_unreserve(Origin::signed(bob), token_id);
                assert_eq!(NftsModule::get_token_reserve(token_id), false);

            });
        }

        #[test]
        fn reserve_safe_transfer_test() {
            new_test_ext().execute_with(|| {
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = <AllNFTsIndex<Test>>::get(0);
                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);

                NftsModule::token_reserve(Origin::signed(bob), token_id);
                let res = NftsModule::_reserve_safe_transfer(bob, alice, token_id);
                assert_eq!(res, Err("token reserved, transfer now is forbidden"));

                NftsModule::token_unreserve(Origin::signed(bob), token_id);
                let res = NftsModule::_reserve_safe_transfer(bob, alice, token_id);
                assert_eq!(res, Ok(()));

            });
        }
        
}