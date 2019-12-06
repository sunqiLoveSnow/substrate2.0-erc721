
use codec::{Encode, Decode};
use system::ensure_signed;
use sr_primitives::traits::{Hash};
use rstd::prelude::*;
use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap,     

};
use sr_primitives::traits::{CheckedAdd, CheckedSub};
use crate::erc721;
use rstd::collections::btree_map::BTreeMap;
// use rstd::collections::btree_set::BTreeSet;
use crate::nfts;
use crate::nfts::{
    BalanceOf
};
use rstd::collections::vec_deque::VecDeque;
use rstd::result;
use rstd::ops::Bound::*;


pub trait Trait: nfts::Trait + timestamp::Trait + generic_asset::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;   
}

// pub type HashOf<T> = <T as system::Trait>::Hash;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
enum OrderStatus{
    Open,
    PartialFilled,
    Filled,
    Closed,
    Canceled,
}
#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum TokenAttrValType{
    String(Vec<u8>),
    Uint64(u64),
}
use crate::order::TokenAttrValType::Uint64;
use crate::order::TokenAttrValType::String; 
use rstd::fmt;
fn unwrap_failed(msg: &str, error: &dyn fmt::Debug) -> ! {
    panic!("{}: {:?}", msg, error)
}
impl TokenAttrValType{
    fn is_uint64(&self) -> bool{
        match *self {
            Uint64(_) => true,
            String(_) => false,
        }
    }
    
    pub fn unwrap_uint64(self) -> u64 {
        match self {
            Uint64(t1) => t1,
            String(t2) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &t2),
        }
    }
    pub fn unwrap_string(self) -> Vec<u8> {
        match self {
            String(t1) => t1,
            Uint64(t2) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &t2),
        }
    }
    
    
}
type TokenAttrType = BTreeMap<Vec<u8>, TokenAttrValType>;

#[derive(Encode, Decode, Clone, PartialEq, PartialOrd, Copy, Debug)]
pub enum CompareOpcode
{
    TokenCmpEq = 0,
    TokenCmpGt,
    TokenCmpLt,
    TokenCmpGe,
    TokenCmpLe,
    TokenCmpNe,
    TokenCmpMax,
}
impl Default for CompareOpcode {
    fn default() -> Self { CompareOpcode::TokenCmpEq }
}

#[derive(Encode, Decode, Clone, PartialEq, PartialOrd, Copy, Debug)]
pub enum LogicOpcode
{
    TokenLogicAnd = 0,
    TokenLogicOr,
    TokenLogicXor,
    TokenLogicMax,
    TokenCmpTrue,
    TokenCmpFalse,
}
impl Default for LogicOpcode {
    fn default() -> Self { LogicOpcode::TokenLogicAnd }
}
#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct BooleanExpression
{
    op: CompareOpcode,
    key: Vec<u8>,
    val : TokenAttrValType,
}
fn _token_attr_match<E: PartialOrd + PartialEq>(comparison:CompareOpcode, v1: E, v2:E) -> bool{
    match comparison
    {
        CompareOpcode::TokenCmpEq=>
            return v1 == v2,
        CompareOpcode::TokenCmpGt=>
            return v1 > v2,
        CompareOpcode::TokenCmpLt=>
            return v1 < v2,
        CompareOpcode::TokenCmpGe=>
            return v1 >= v2,
        CompareOpcode::TokenCmpLe=>
            return v1 <= v2,
        CompareOpcode::TokenCmpNe=>
            return v1 != v2,
        _ => return false
    }
}
fn _filter_match( attr: &TokenAttrType, filter: &BooleanExpression ) -> bool
{
    let a = match attr.get(&filter.key){
        Some(x) => x,
        None => return false,
    };
    let b = &filter.val;
    if a.is_uint64() ^ b.is_uint64() {
        return false;
    };
    if a.is_uint64() {
        let v1 = a.clone().unwrap_uint64();
        let v2 = b.clone().unwrap_uint64();
        return _token_attr_match(filter.op, v1, v2);
    }else{
        let v1 = a.clone().unwrap_string();
        let v2 = b.clone().unwrap_string();
        return _token_attr_match(filter.op, v1, v2);
    };
}




#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub enum FilterItem {
    Uint8T(LogicOpcode), 
    BoolExp(BooleanExpression),
}
use crate::order::FilterItem::Uint8T;
use crate::order::FilterItem::BoolExp; 
impl FilterItem{
    fn is_uint8_t(&self) -> bool{
        match *self {
            Uint8T(_) => true,
            BoolExp(_) => false,
        }
    }
    pub fn unwrap_uint8_t(self) -> LogicOpcode {
        match self {
            Uint8T(t1) => t1,
            BoolExp(t2) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &t2),
        }
    }
    pub fn unwrap_bool_exp(self) -> BooleanExpression {
        match self {
            BoolExp(t1) => t1,
            Uint8T(t2) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &t2),
        }
    }
}

type FilterStack = Vec<FilterItem> ;

#[derive(Clone, Debug)]
struct TokenParser {
    s:VecDeque<LogicOpcode>,
}


impl TokenParser{
    // fn foo(){}
    fn append_op(&mut self, op:LogicOpcode )-> Result{
        if op < LogicOpcode::TokenLogicMax{
            if op == LogicOpcode::TokenLogicXor{
                return Err("Xor is not supported now")
            }
            self.s.push_back(op);
        }else{
            return Err("Invalid logic comparison operator")
        }
        return Ok(())
    }
    fn append_boolean(&mut self, result: bool) ->Result {
        match result {
            true => {
                self.s.push_back(LogicOpcode::TokenCmpTrue);
            },
            false =>{
                self.s.push_back(LogicOpcode::TokenCmpFalse);
            },
        };
        match self.reduce(){
            Err(x) => return Err(x),
            Ok(_) => return Ok(())
        }
    }
    fn reduce(&mut self) -> Result{
        loop {
            let siz = self.s.len();
            if siz < 3{
                return Ok(())
            };
            let top1 = *self.s.get(siz -1).unwrap();
            if top1 < LogicOpcode::TokenLogicMax {
                return Ok(())
            };
            let top2 = *self.s.get(siz - 2).unwrap();
            if top2 < LogicOpcode::TokenLogicMax {
                return Ok(())
            };
            let top3 = *self.s.get(siz - 3).unwrap();
            if top3 >= LogicOpcode::TokenLogicMax {
                return Err("Invalid dfs stack, logic comparison expected")
            };
            let mut new_top = LogicOpcode::TokenCmpFalse;
            match top3{
                LogicOpcode::TokenLogicAnd => {
                    if (top1 == LogicOpcode::TokenCmpTrue) && (top2 == LogicOpcode::TokenCmpTrue){
                        new_top = LogicOpcode::TokenCmpTrue;
                        
                    };
                    // return Ok(())
                },
                LogicOpcode::TokenLogicOr => {
                    if(top1 == LogicOpcode::TokenCmpTrue) || (top2 == LogicOpcode::TokenCmpTrue){
                        new_top = LogicOpcode::TokenCmpTrue;
                    };
                    // return Ok(())
                },
                LogicOpcode::TokenLogicXor => {
                    if(top1 == LogicOpcode::TokenCmpTrue) ^ (top2 == LogicOpcode::TokenCmpTrue){
                        new_top = LogicOpcode::TokenCmpTrue;
                    }
                    // return Ok(())
                },
                _ => (),
                
            };// end match
            self.s.pop_back();
            self.s.pop_back();
            self.s.pop_back();
            self.s.push_back(new_top);
            

        };// end while
        // Ok(())
    }

    fn finish(&mut self)-> result::Result<bool, &'static str>{
        if self.s.len() != 1 {
            return Err("Can not reduce stack to single element")
        };
        let top = *self.s.back().unwrap();
        if !((top == LogicOpcode::TokenCmpTrue) || (top ==LogicOpcode::TokenCmpFalse)){
            return Err("Can not reduce stack to boolean")
        };
        self.s.pop_back();
        return Ok(top == LogicOpcode::TokenCmpTrue)
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct TokenAttrSelector
{
    max_count: u32,
    stack:FilterStack,
}
impl TokenAttrSelector{
    
    fn validate(&self) -> Result{
        let mut parser= TokenParser{
            s:VecDeque::<LogicOpcode>::new(),
        };
        if self.stack.len() < 20{
            if self.stack.len() == 0{
                return Ok(())
            }
            if self.max_count <= 0 {
                return Err("")
            };
            // if self.max_amount <= 0{
            //     return Err("")
            // };
            for item in self.stack.iter(){
                match item{
                    Uint8T(ref x) => {
                        match parser.append_op(*x){
                            Err(e) => return Err(e),
                            Ok(()) => {},
                        };
                    },
                    BoolExp(ref y) => {
                        if y.op >= CompareOpcode::TokenCmpMax {
                            return Err("")
                        };
                        match parser.append_boolean(true){
                            Err(e) => return Err(e),
                            Ok(()) => {},
                        };                        
                    },
                }
                
            };
            match parser.finish(){
                Err(e) => return Err(e),
                Ok(_) => {},
            }
            
        }else {
            return Err("Cannot set more than 10 filters")
        }
        return Ok(())
    }
}


#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub struct TokenIdSelector<Hash> 
{
    // id_set: BTreeSet<Hash>,// token_id_type : H::Hash
    id_set: Vec<Hash>,// token_id_type : H::Hash
}
impl<Hash> TokenIdSelector<Hash>{
    fn validate(&self) -> Result{
        if self.id_set.len() > 0{
            return Ok(())
        }else {
            return Err("Id set can not be empty")
        }
    }

}
#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub enum SelectorType<Hash> 
    // T: Trait
{
    IdSelect(TokenIdSelector<Hash>),
    AttrSelect(TokenAttrSelector),
}


use crate::order::SelectorType::IdSelect;
use crate::order::SelectorType::AttrSelect;
use core::convert::TryInto;



fn _token_selector_match(attr: &TokenAttrType, selector: &TokenAttrSelector) ->bool
{ 
    if selector.stack.len() == 0 { // empty stack means fit all token
        return true;
    }

    let mut parser = TokenParser{
        s:VecDeque::<LogicOpcode>::new(),
    } ;
    for item in selector.stack.iter()
    {
        if item.is_uint8_t(){
            match parser.append_op(item.clone().unwrap_uint8_t()){
                Ok(_) => {},
                Err(e) => return false,
            };
        } else {
            let filter = item.clone().unwrap_bool_exp();
            match parser.append_boolean(_filter_match(attr, &filter)){
                Ok(_) => {},
                Err(e) => return false,
            };
        }
    };
    match parser.finish(){
        Ok(x) => return x,
        Err(_) => return false,
    }
}




impl<Hash> SelectorType<Hash>
{

    pub fn token_count(&self) -> u32 {
        match *self {
            IdSelect(ref x) => {
                let ret:u32= x.id_set.len().try_into().unwrap();
                return ret
            },
            AttrSelect(ref y) => {
                return y.max_count
            },
        }
    }
    pub fn validate(&self) -> Result{
        match *self {
            IdSelect(ref x) => {
                x.validate()
            },
            AttrSelect(ref y) => {
                y.validate()
            },
        }
    }
    pub fn is_filter_by_id_set(&self) -> bool {
        match *self {
            IdSelect(_) => true,
            AttrSelect(_) => false,
        }
    }
}

#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub struct TokenSelector<Hash> 
{
    selector: SelectorType<Hash>,
    nft_type: Hash,

}
impl<Hash> TokenSelector<Hash>
{

    fn token_count(&self) -> u32 {
        self.selector.token_count()
    }
    fn validate(&self) ->Result {
        self.selector.validate()
    }
    fn is_filter_by_id_set(&self) -> bool {
        self.selector.is_filter_by_id_set()
    }
}



#[derive(Encode, Decode, Clone, Debug)]
pub struct AskOrderItem<T> where T:Trait{
    creator: T::AccountId,
    order_id: T::Hash,
    selector: TokenSelector<T::Hash>,
    asset: T::AssetId,
    price: T::Balance,
    timepoint: T::Moment,
    fill_or_kill: bool,
    bind_tokens: Vec<T::Hash>,
    status: OrderStatus,
}
impl<T:Trait> core::fmt::Display for AskOrderItem<T>{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n", 
        self.creator, 
        self.order_id, 
        self.asset, 
        self.price, 
        self.timepoint, 
        self.fill_or_kill, 
        self.bind_tokens.len(), 
        self.status  )
    }
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct BidOrderItem<T> where T:Trait{
    creator: T::AccountId,
    order_id: T::Hash,
    selector: TokenSelector<T::Hash>,
    asset: T::AssetId,
    price: T::Balance,
    timepoint: T::Moment,
    fill_or_kill: bool,
    tk_count_to_buy: BalanceOf<T>,
    status: OrderStatus,
}

impl<T:Trait> core::fmt::Display for BidOrderItem<T>{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n", 
        self.creator, 
        self.order_id, 
        self.asset, 
        self.price, 
        self.timepoint, 
        self.fill_or_kill, 
        self.tk_count_to_buy, 
        self.status  )
    }
}


#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub struct TokenPrice<T> where T:Trait{
    asset: T::AssetId,
    amount: T::Balance,
}
#[derive(Encode, Decode, Clone, Debug, PartialEq)]
pub struct Attributes{
    key: Vec<u8>,
    value: TokenAttrValType,
}

decl_event!(
    pub enum Event<T>
    where
        <T as system::Trait>::AccountId,
        <T as system::Trait>::Hash,
        <T as generic_asset::Trait>::Balance,
        <T as timestamp::Trait>::Moment,
        <T as generic_asset::Trait>::AssetId
    {
        // TokenOrderCreate(AccountId, Hash), 

        // creator, order_id, asset, price, timepoint, fill_or_kill
        OrderOpened(AccountId, Hash, AssetId, Balance, Moment, bool),
        // seller, buyer, token_id,trade_asset, trade_price, timepoint, 
        OrderFilled(AccountId, AccountId, Hash, AssetId, Balance, Moment),
        // creator, order_id, timepoint, 
        OrderCanceled(AccountId, Hash, Moment),
        // creator, order_id, asset, price, timepoint, fill_or_kill
        OrderClosed(AccountId, Hash, AssetId, Balance, Moment, bool),
    }
);


decl_storage! {
    trait Store for Module<T: Trait> as NFTStorage {
        // token_id => token Attributes btreemap(String -> enum(String, integer) )
        TokenAttribuites get(get_token_attr) : map T::Hash => TokenAttrType;
        // order_id =>  AskOrderItem
        AskTokenOrders get(get_ask_token_order) : map T::Hash => Option<AskOrderItem<T>>;
        // order_id => bidorderitem
        BidTokenOrders get(get_bid_token_order) : map T::Hash => Option<BidOrderItem<T>>;
        // orderbook[asset]     price => order_id vec
        AskOrderBook get(get_orderbook_ask): map T::AssetId => BTreeMap<T::Balance, Vec<T::Hash>>;
        BidOrderBook get(get_orderbook_bid): map T::AssetId => BTreeMap<T::Balance, Vec<T::Hash>>;
        // order id => account
        // OrderOwner get(get_order_owner): map T::Hash => T::AccountId;
        // (account, index ) => order_id
        // account_id => vec<order_id>
        // OwnedOrders get(get_orders_owned): map T::AccountId => Vec<T::Hash>;
        // Nonce: u64;
    }
}

decl_module! {
    
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        fn set_token_attr(origin, token_id:T::Hash , attribute: Attributes ) -> Result{
            let sender = ensure_signed(origin)?;
            Self::_set_token_attr(sender, token_id, attribute)
        }
        fn rmv_token_attr(origin, token_id:T::Hash , attribute_key: Vec<u8> ) -> Result{
            let sender = ensure_signed(origin)?;
            Self::_rmv_token_attr(sender, token_id, attribute_key)
        }
        fn token_buy_order_create(origin,
            selector: TokenSelector<T::Hash>,
            asset: T::AssetId,
            price: T::Balance,
            fill_or_kill: bool) -> Result{
            let creator = ensure_signed(origin)?;
            Self::_token_buy_order_create(creator, selector, asset, price, fill_or_kill)
        }
        fn token_sell_order_create(origin,
            selector: TokenSelector<T::Hash>,
            asset: T::AssetId,
            price: T::Balance,
            fill_or_kill: bool) -> Result{
            let creator = ensure_signed(origin)?;
            Self::_token_sell_order_create(creator, selector, asset, price, fill_or_kill)
        }
        
        fn token_buy_order_cancel(origin, order_id:T::Hash) -> Result{
            let creator = ensure_signed(origin)?;
            Self::_token_buy_order_cancel(creator,order_id)
        }
        
        fn token_sell_order_cancel(origin, order_id:T::Hash) -> Result{
            let creator = ensure_signed(origin)?;
            Self::_token_sell_order_cancel(creator, order_id)
        }
    }

    
}


impl<T: Trait> Module<T> {
    
    fn _set_token_attr(sender: T::AccountId, token_id:T::Hash , attribute: Attributes ) -> Result{
        // let mut token_attributes = Self::get_token_attr(&token_id);
        // token_attributes.insert(attribute.key, attribute.value);
        let owner = <erc721::Module<T>>::owner_of(&token_id);
        if owner.is_none() {
            return Err("token not found")
        };
        ensure!(owner.unwrap() == sender, "sender is not the owner of this token");
        <TokenAttribuites<T>>::mutate(token_id, |x| x.insert(attribute.key, attribute.value));
        Ok(())
    }
    fn _rmv_token_attr(sender: T::AccountId, token_id:T::Hash , attribute_key: Vec<u8> ) -> Result{
        // <nfts::Module<T>>::
        // let mut token_attributes = Self::get_token_attr(&token_id);
        // token_attributes.remove(&attribute_key);
        let owner = <erc721::Module<T>>::owner_of(&token_id);
        if owner.is_none() {
            return Err("token not found")
        };
        ensure!(owner.unwrap() == sender, "sender is not the owner of this token");
        match <TokenAttribuites<T>>::mutate(token_id, |x| x.remove(&attribute_key)){
            None => return Err("attribute key not found"),
            Some(_) => return Ok(())
        }
    }
    fn _token_match_visitor(sender:T::AccountId, selector: &TokenSelector<T::Hash>) ->  Vec<T::Hash>{ // add to bind_tokens
        // let owner_tokens = <nfts::Module<T>>::get_nfts_owner_vec(&sender);
        let own_count = <erc721::Module<T>>::balance_of(&sender);
        
        let mut bind_tokens = Vec::<T::Hash>::new();
        match &selector.selector {
            IdSelect(t) => {
                let mut idx = own_count;
                loop {
                    let new_idx = idx.checked_sub(&1.into());
                    if new_idx.is_none(){
                        break;
                    };
                    idx = new_idx.unwrap();

                    let token_id =  <erc721::Module<T>>::token_of_owner_by_index((&sender,idx));
                    if token_id == T::Hash::default(){
                        continue;
                    };
                    if <nfts::Module<T>>::get_token_reserve(token_id){
                        continue; // escape if token reserved
                    };
                    match t.id_set.iter().position(|x| *x == token_id){
                        None => {},
                        Some(_) => bind_tokens.push(token_id),
                    };
                    
                }
            },
            AttrSelect(subselector) => {
                let mut idx = own_count;
                loop {
                    let new_idx = idx.checked_sub(&1.into());
                    if new_idx.is_none(){
                        break;
                    };
                    idx = new_idx.unwrap();
                    let token_id =  <erc721::Module<T>>::token_of_owner_by_index((&sender,idx));
                    if token_id == T::Hash::default(){
                        continue;
                    };
                    let token_attr = Self::get_token_attr(&token_id);

                    let token = <nfts::Module<T>>::get_token(&token_id);
                    if token.is_none() {
                        continue;
                    };
                    let token = token.unwrap();
                    if token.nft_id != selector.nft_type  {
                        continue;
                    };
                    if <nfts::Module<T>>::get_token_reserve(token_id){
                        continue; // escape if token reserved
                    };
                    if _token_selector_match(&token_attr, &subselector) {
                        bind_tokens.push(token_id);
                    }
                    
                };
            }, 
        }
        
        bind_tokens
    }
    fn _remove_order_from_orderbook(fill_price:T::Balance, fill_asset:T::AssetId, order_id: T::Hash,  is_bid :bool)-> Result{
        let orders_map = match is_bid {
            false => Self::get_orderbook_ask(&fill_asset),
            true => Self::get_orderbook_bid(&fill_asset),
        };
        if is_bid {
            let order = match Self::get_bid_token_order(&order_id){
                Some(t) => t,
                None => return Err("order not found"),
            };
            // order.status = OrderStatus::Closed;
            Self::deposit_event(RawEvent::OrderClosed(order.creator, order_id, order.asset, order.price, order.timepoint, order.fill_or_kill));
            // remove order from BidTokenOrders
            <BidTokenOrders<T>>::remove(&order_id);

            let order_vec = match orders_map.get(&fill_price){
                Some(x) => x,
                None => return Err("")
            };
            let pos = order_vec.iter().position(|x| *x == order_id);
            if pos.is_none(){
                return Err("")
            };
            let pos = pos.unwrap();

            <BidOrderBook<T>>::mutate(&fill_asset, |x|{
                x.get_mut(&fill_price).unwrap().remove(pos);
            });
            
        }else{
            let order = match Self::get_ask_token_order(&order_id){
                Some(t) => t,
                None => return Err("order not found"),
            };
            // order.status = OrderStatus::Closed;
            Self::deposit_event(RawEvent::OrderClosed(order.creator, order_id, order.asset, order.price, order.timepoint, order.fill_or_kill));
            // remove order from AskTokenOrders
            <AskTokenOrders<T>>::remove(&order_id);

            let order_vec = match orders_map.get(&fill_price){
                Some(x) => x,
                None => return Err("")
            };
            let pos = order_vec.iter().position(|x| *x == order_id);
            
            if pos.is_none(){
                return Err("")
            };
            let pos = pos.unwrap();

            <AskOrderBook<T>>::mutate(&fill_asset, |x|{
                x.get_mut(&fill_price).unwrap().remove(pos);
            });
        };
                      
        Ok(())
        // event fill 
    }
    fn _reserve_asset(owner:T::AccountId ,asset:T::AssetId, amount: T::Balance) -> Result{
        <generic_asset::Module<T>>::reserve(&asset, &owner, amount)
    }

    fn _unreserve_asset(owner:T::AccountId ,asset:T::AssetId, amount: T::Balance) -> Result{
        let delta_between_actural_and_input = <generic_asset::Module<T>>::unreserve(&asset, &owner, amount);
        // to do with delta...
        Ok(())
    }
    fn _transfer_asset(from:T::AccountId, to: T::AccountId, amount: T::Balance, asset:T::AssetId)-> Result{
        if from == to{
            return Ok(())
        };
        <generic_asset::Module<T>>::make_transfer(&asset, &from, &to, amount)?;
        Ok(())
    }
    
    fn _fill(fill_price:T::Balance, fill_asset:T::AssetId, order_id: T::Hash, token_id: T::Hash, buyer:T::AccountId, seller:T::AccountId, is_bid: bool) ->Result {
        let timepoint = <timestamp::Module<T>>::get() ;
        Self::deposit_event(RawEvent::OrderFilled(seller.clone(), buyer.clone(), token_id, fill_asset, fill_price, timepoint));
       
        if is_bid == false{// ask order
            // let mut order = match Self::get_ask_token_order(&order_id){
            let order = match Self::get_ask_token_order(&order_id){
                Some(tt)=> tt,
                None => return Err("")

            };
            // order.status = OrderStatus::PartialFilled;
            // let ref mut bind_tokens = order.bind_tokens;
            let bind_tokens = order.bind_tokens;
            let pos = bind_tokens.iter().position(|x| *x == token_id);
            if pos.is_none(){
                return Err("token not found")
            };
            // unreserve token
            <nfts::Module<T>>::_token_unreserve(seller.clone(), token_id)?;
            // exchange token with money
            <nfts::Module<T>>::_reserve_safe_transfer(seller.clone(), buyer.clone() , token_id)?;

            Self::_unreserve_asset(buyer.clone(), fill_asset, fill_price)?;
            
            match Self::_transfer_asset(buyer.clone(), seller.clone(), fill_price, fill_asset){
                Ok(_) => {},
                Err(e) => return Err(e)
            };
            let mut will_remove = false;
            <AskTokenOrders<T>>::mutate(&order_id, |x| {
                match x {
                    Some(xx) => {
                        xx.status = OrderStatus::PartialFilled;
                        xx.bind_tokens.remove(pos.unwrap());
                        if xx.bind_tokens.len() == 0 {
                            xx.status = OrderStatus::Filled;
                            // Self::_remove_order_from_orderbook(fill_price, fill_asset, order_id, is_bid);
                            will_remove = true;
                        } ;
                    },
                    None => {},
                }; 
                            
            });
            if will_remove{
                Self::_remove_order_from_orderbook(fill_price, fill_asset, order_id, is_bid)?;
            }

        }else{
            // let mut order = match Self::get_bid_token_order(&order_id){
            let order = match Self::get_bid_token_order(&order_id){
                Some(t) => t,
                None => return Err("")
            };
            // order.status = OrderStatus::PartialFilled;
            let mut tk_count_to_buy = order.tk_count_to_buy;
            tk_count_to_buy -= 1.into();
            // unreserve token
            <nfts::Module<T>>::_token_unreserve(seller.clone(), token_id)?;
            // exchange token with money
            match <nfts::Module<T>>::_reserve_safe_transfer(seller.clone(), buyer.clone() , token_id){
                Ok(_) => {},
                Err(e) => return Err(e)
            };
            Self::_unreserve_asset(buyer.clone(), fill_asset, fill_price)?;
            
            match Self::_transfer_asset(buyer.clone(), seller.clone(), fill_price, fill_asset){
                Ok(_) => {},
                Err(e) => return Err(e)
            };
            let mut will_remove = false;
            <BidTokenOrders<T>>::mutate(&order_id, |x| {
                match x {
                    Some(xx) => {
                        xx.status = OrderStatus::PartialFilled;
                        xx.tk_count_to_buy = tk_count_to_buy;
                        // *x = Some(*xx); 
                        if tk_count_to_buy <= 0.into() {
                            xx.status = OrderStatus::Filled;
                            will_remove = true;
                            // Self::_remove_order_from_orderbook(fill_price, fill_asset, order_id, is_bid);
                            
                        } ;
                    },
                    None => {},
                }; 
                            
            });
            if will_remove{
                Self::_remove_order_from_orderbook(fill_price, fill_asset, order_id, is_bid)?;
            }

        }
        Ok(())
    }
    fn _token_match_bid(buyer: T::AccountId, bid_price: T::Balance, bid_asset: T::AssetId, mut tk_count_to_buy: u32 , selector: &TokenSelector<T::Hash> ) -> u32{ // if fill remove from bind_tokens
        // let mut amount = 0;
        let orderbook = Self::get_orderbook_ask(&bid_asset);
        // let mut amount_to_pay: T::Balance = tk_count_to_buy * bid_price;
        // let mut amount_payed = 0;

        for (&price, order_vec) in orderbook.range((Unbounded, Included(&bid_price))) {
            // println!("{}: {}", key, value);
            for &order_id in order_vec.iter(){// it: order_id
                let order = Self::get_ask_token_order(order_id);
                if order.is_none(){
                     continue;
                };
                let order = order.unwrap();
                let mut bind_tokens = order.bind_tokens;
                let seller = order.creator;

                // let bind_tokens_copy = bind_tokens.clone();
                let mut i:usize = 0;
                // let maker_price = *price;// maker price

                for &token_id in bind_tokens.clone().iter(){
                    match &selector.selector {
                        IdSelect(t) => {
                            if t.id_set.contains(&token_id) {
                                // fill
                                match Self::_fill(price, bid_asset, order_id, token_id, buyer.clone(), seller.clone(), false){
                                    Ok(_) => {},
                                    Err(_) => continue,
                                };
                                // bind_tokens.remove(i);
                                if let Some(elem) = bind_tokens.get_mut(i) {
                                    *elem = T::Hash::default();
                                };
                                tk_count_to_buy -= 1;
                                if tk_count_to_buy <= 0 {
                                    return tk_count_to_buy
                                };
                            }
                        },
                        AttrSelect(subselector) => {
                            let token_attr = Self::get_token_attr(token_id);
                            let token = match <nfts::Module<T>>::get_token(token_id){
                                Some(t) => t,
                                None => {
                                    i += 1;
                                    continue
                                },
                            };
                            if token.nft_id != selector.nft_type {
                                i += 1;
                                continue
                            };
                            if _token_selector_match(&token_attr, &subselector) {
                                // fill
                                match Self::_fill(price, bid_asset, order_id, token_id, buyer.clone(), seller.clone(), false){
                                    Ok(_) => {},
                                    Err(_) => continue,
                                };
                                // bind_tokens.remove(i);
                                if let Some(elem) = bind_tokens.get_mut(i) {
                                    *elem = T::Hash::default();
                                };
                                tk_count_to_buy -= 1;
                                if tk_count_to_buy <= 0 {
                                    return tk_count_to_buy
                                };
                            }
                            
                        }, 
                    };
                    i += 1;
                };
                    
            }
        }
        return tk_count_to_buy
        
    }
    fn _token_match_ask(seller: T::AccountId, ask_price: T::Balance, ask_asset: T::AssetId, mut bind_tokens: Vec<T::Hash> ) -> Vec<T::Hash>{ // if fill remove from bind_tokens
        let orderbook = Self::get_orderbook_bid(&ask_asset);
        for (&price, order_vec) in orderbook.range((Included(&ask_price), Unbounded )).rev() {
            for &order_id in order_vec.iter(){// it: order_id
                if bind_tokens.len() == 0{
                    break;
                }
                let order = match Self::get_bid_token_order(order_id){
                    Some(t) => t,
                    None => continue,
                };
                let buyer = match Self::get_bid_token_order(order_id){
                    Some(t) => t.creator,
                    None => continue,
                };
                // let mut amount_to_pay = order.amount_to_pay;
                let mut tk_count_to_buy = order.tk_count_to_buy;
                let selector = order.selector;
                // let maker_price = *price;// maker price
                // let bind_tokens_copy = bind_tokens.clone();
                let mut i:usize = 0;
                
                for &token_id in bind_tokens.clone().iter(){
                    match &selector.selector {
                        IdSelect(t) => {
                            if t.id_set.contains(&token_id) {
                                // send fill op
                                match Self::_fill(price, ask_asset, order_id, token_id, buyer.clone(), seller.clone(), true){
                                    Ok(_) => {},
                                    Err(_) => continue,
                                };
                                // bind_tokens.remove(i);
                                if let Some(elem) = bind_tokens.get_mut(i) {
                                    *elem = T::Hash::default();
                                };
                                // amount_to_pay -= maker_price;
                                tk_count_to_buy -= 1.into();
                                if tk_count_to_buy <= 0.into() {
                                    // conterpart bid order filled and finished, should remove from bid orderbook
                                    break;
                                };
                                
                            }
                        },
                        AttrSelect(subselector) => {
                            let token_attr = Self::get_token_attr(token_id);
                            let token = match <nfts::Module<T>>::get_token(token_id){
                                Some(t) => t,
                                None => {
                                    i += 1;
                                    continue
                                },
                            };
                            if token.nft_id != selector.nft_type {
                                i += 1;
                                continue
                            };
                            if _token_selector_match(&token_attr, &subselector) {
                                // fill
                                match Self::_fill(price, ask_asset, order_id, token_id, buyer.clone(), seller.clone(), true){
                                    Ok(_) => {},
                                    Err(_) => continue,
                                };
                                // bind_tokens.remove(i);
                                if let Some(elem) = bind_tokens.get_mut(i) {
                                    *elem = T::Hash::default();
                                };
                                // amount_to_pay -= maker_price;
                                tk_count_to_buy -= 1.into();
                                if tk_count_to_buy <= 0.into() {
                                    // conterpart bid order filled and finished, should remove from bid orderbook
                                    break;
                                }
                            }
                            
                        }, 
                    };
                    i += 1;
                };
            }
        };
        bind_tokens.retain(|&x| x != T::Hash::default());
        return bind_tokens
    }
    
    fn _try_init_orderbook(asset:T::AssetId, is_bid: bool) -> Result {
        match is_bid{
            true => {
                let orderbook = Self::get_orderbook_bid(&asset);
                if orderbook.is_empty(){
                    let new_tree = BTreeMap::<T::Balance, Vec<T::Hash>>::new();
                    // new_tree.insert(<T::Balance>::min_value(), Vec::<T::Hash>:new());
                    <BidOrderBook<T>>::insert(asset, new_tree);
                };
            },
            false => {
                let orderbook = Self::get_orderbook_ask(&asset);
                if orderbook.is_empty(){
                    let new_tree = BTreeMap::<T::Balance, Vec<T::Hash>>::new();
                    // new_tree.insert(<T::Balance>::min_value(), Vec::<T::Hash>:new());
                    <AskOrderBook<T>>::insert(asset, new_tree);
                };
            },
        };
        Ok(())
        
    }
    fn _try_add_order_to_orderbook(price : T::Balance, asset:T::AssetId, order_id:T::Hash, is_bid: bool) -> Result{
        if !is_bid{
            let orderbook = Self::get_orderbook_ask(&asset);
            if orderbook.contains_key(&price) == false{
                let new_vec = Vec::<T::Hash>::new();
                <AskOrderBook<T>>::mutate(asset,|x| x.insert(price, new_vec) );
            };
            <AskOrderBook<T>>::mutate(asset,|x| x.get_mut(&price).unwrap().push(order_id) );
        
        }else{
            let orderbook = Self::get_orderbook_bid(&asset);
            if orderbook.contains_key(&price) == false{
                let new_vec = Vec::<T::Hash>::new();
                <BidOrderBook<T>>::mutate(asset,|x| x.insert(price, new_vec) );
            };
            <BidOrderBook<T>>::mutate(asset,|x| x.get_mut(&price).unwrap().push(order_id) );
        };
        
        Ok(())
    }
    fn _token_sell_order_create(creator: T::AccountId,
        selector: TokenSelector<T::Hash>,
        asset: T::AssetId,
        price: T::Balance,
        fill_or_kill: bool) -> Result {

        let timepoint = <timestamp::Module<T>>::get() ;
        let order_id = (&creator, &asset, timepoint,  price, fill_or_kill, true, &selector).using_encoded(<T as system::Trait>::Hashing::hash);

        // trigger match, send fill op if filled
        let bind_tokens = Self::_token_match_visitor(creator.clone(), &selector);
        ensure!(bind_tokens.len() > 0, "no token selected, so invalid, please check your token_id and Attributes and reset");
        // let token_upper_limit_size: usize = selector.token_count().into();
        ensure!(bind_tokens.len() <= selector.token_count().try_into().unwrap(), "upper limit amount of bind tokens is exceeded");
        Self::deposit_event(RawEvent::OrderOpened(creator.clone(), order_id, asset, price, timepoint, fill_or_kill));

        // reserve tokens
        for token_id in bind_tokens.iter(){
            <nfts::Module<T>>::_token_reserve(creator.clone(), *token_id)?;
        };
        
        // remove from bind_tokens if fill
        let bind_tokens = Self::_token_match_ask(creator.clone(), price, asset, bind_tokens);
        // add left to token ask orderbook if not fill_or_kill
        if bind_tokens.len() ==0 || fill_or_kill {
            // unreserve left tokens
            for token_id in bind_tokens.iter(){
                <nfts::Module<T>>::_token_unreserve(creator.clone(), *token_id)?;
            };
            Self::deposit_event(RawEvent::OrderClosed(creator.clone(), order_id, asset, price, timepoint, fill_or_kill));

            return Ok(())
        };

        let order = AskOrderItem{
            creator:creator.clone(),
            order_id,
            selector,
            asset,
            price,
            timepoint,
            fill_or_kill,
            bind_tokens:bind_tokens.clone(),
            status: OrderStatus::Open,
        };
        
        // wirte to tokenorders
        <AskTokenOrders<T>>::insert(order_id, order);
        // add to orderbook
        Self::_try_init_orderbook(asset, false)?;// add asset entry
        Self::_try_add_order_to_orderbook(price, asset, order_id, false)?; // add price entry

        Ok(())
        
    }
    fn _token_buy_order_create(creator: T::AccountId,
        selector: TokenSelector<T::Hash>,
        asset: T::AssetId,
        price: T::Balance,
        fill_or_kill: bool) -> Result {

        let timepoint = <timestamp::Module<T>>::get() ;
        let order_id = (&creator, &asset, timepoint, price, fill_or_kill, false, &selector).using_encoded(<T as system::Trait>::Hashing::hash);
        // send order create event
        Self::deposit_event(RawEvent::OrderOpened(creator.clone(), order_id, asset, price, timepoint, fill_or_kill));

        let tk_count_to_buy = selector.token_count();
        // reserve asset
        let tk_count_to_buy_balance : T::Balance = tk_count_to_buy.into();
        let reserve_amount = tk_count_to_buy_balance * price;
        Self::_reserve_asset(creator.clone(), asset, reserve_amount)?;
        
        // trigger match, send fill op if filled
        let tk_count_to_buy = Self::_token_match_bid(creator.clone(), price, asset, tk_count_to_buy, &selector);
        // add left to token bid orderbook if not fill_or_kill
        if tk_count_to_buy == 0 || fill_or_kill {
            let tk_count_to_buy_balance : T::Balance = tk_count_to_buy.into();
            let unreserved_balance = tk_count_to_buy_balance * price;
            return Self::_unreserve_asset(creator.clone(), asset, unreserved_balance );
            // return Ok(())
        };
        
        let order = BidOrderItem{
            creator:creator.clone(),
            order_id,
            selector,
            asset,
            price,
            timepoint,
            fill_or_kill,
            tk_count_to_buy: tk_count_to_buy.into() ,
            status: OrderStatus::Open,
        };
        
        // wirte to tokenorders
        <BidTokenOrders<T>>::insert(order_id, order);
        // add to orderbook
        Self::_try_init_orderbook(asset, true)?;// add asset entry
        Self::_try_add_order_to_orderbook(price, asset, order_id, true)?; // add price entry

        Ok(())
        
    }
    fn _token_sell_order_cancel(creator : T::AccountId, order_id:T::Hash)->Result{
        let order = match Self::get_ask_token_order(&order_id) {
            Some(t) => t,
            None => return Err("")
        };
        ensure!(order.creator == creator,"creator dismatch, creator not own this order");
        let timepoint = <timestamp::Module<T>>::get() ;
        Self::deposit_event(RawEvent::OrderCanceled(creator.clone(), order_id, timepoint));
        // remove from orderbook
        Self::_remove_order_from_orderbook(order.price, order.asset, order_id, false)?;
        // remove from orderitems
        <AskTokenOrders<T>>::remove(order_id);
        Ok(())
    }
    fn _token_buy_order_cancel(creator : T::AccountId, order_id:T::Hash)->Result{
        let order = match Self::get_bid_token_order(&order_id) {
            Some(t) => t,
            None => return Err("order not found")
        };
        ensure!(order.creator == creator,"creator dismatch, creator not own this order");
        let timepoint = <timestamp::Module<T>>::get() ;
        Self::deposit_event(RawEvent::OrderCanceled(creator.clone(), order_id, timepoint));
        // remove from orderbook
        Self::_remove_order_from_orderbook(order.price, order.asset, order_id, true)?;
        // remove from orderitems
        <BidTokenOrders<T>>::remove(order_id);
        Ok(()) 
    }
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
        impl nfts::Trait for Test{
            type Event = ();
            type Currency = balances::Module<Self>;
        }
        impl generic_asset::Trait for Test {
                type Event = ();
                type Balance = u64;
                type AssetId = u32;
        }
        parameter_types! {
            pub const MinimumPeriod: u64 = 5;

        }
        impl timestamp::Trait for Test {
                type Moment = u64;
                type OnTimestampSet = ();
                type MinimumPeriod = MinimumPeriod;
        }


        impl Trait for Test {
            type Event = ();
        }

        // This function basically just builds a genesis storage key/value store according to
        // our desired mockup.
        fn new_test_ext() -> runtime_io::TestExternalities {
            system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
        }

        pub type NftsModule = nfts::Module<Test>;
        pub type AssetsModule = generic_asset::Module<Test>;
        pub type Erc721Module = erc721::Module<Test>;
        pub type OrderModule = Module<Test>;

        

        use rstd::str;
        #[test]
        fn set_token_attr_test() {
            new_test_ext().execute_with(|| {

                fn print_attributes(attr_value: &TokenAttrValType ){
                    match &attr_value{
                        String(x1) => {// Vec<u8>
                            println!("String as => {}", str::from_utf8(&x1).unwrap());
                        },
                        Uint64(x2) => {
                            println!("integer as => {}", x2);
                        },
                    }
                    
                };
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = NftsModule::get_nft_by_index(0);

                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);
                let attr1 = Attributes{
                    key: "attr1".as_bytes().to_vec(),
                    value: String("attr1_value1".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "attr2".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),token_id, attr1, );
                assert_eq!(res, Ok(()) );
                let res = OrderModule::set_token_attr(Origin::signed(bob),token_id, attr2, );
                assert_eq!(res, Ok(()) );
                let token_attrs = OrderModule::get_token_attr(token_id);
                println!("after add Attributes...");
                for (k, v )in token_attrs.iter().rev(){
                    println!("key : {}", str::from_utf8(&k).unwrap());
                    print_attributes(v);
                };
                let res = OrderModule::rmv_token_attr(Origin::signed(bob),token_id,  "attr1".as_bytes().to_vec() );
                assert_eq!(res, Ok(()) );
                println!("after remove Attributes...");
                let token_attrs = OrderModule::get_token_attr(token_id);
                for (k, v )in token_attrs.iter().rev(){
                    println!("key : {}", str::from_utf8(&k).unwrap());
                    print_attributes(v);
                };

                
            });
        }

        #[test]
        fn token_selector_test() {
            new_test_ext().execute_with(|| {

                fn print_attributes(attr_value: &TokenAttrValType ){
                    match &attr_value{
                        String(x1) => {// Vec<u8>
                            println!("String as => {}", str::from_utf8(&x1).unwrap());
                        },
                        Uint64(x2) => {
                            println!("integer as => {}", x2);
                        },
                    }
                    
                };
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = NftsModule::get_nft_by_index(0);

                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);
                let attr1 = Attributes{
                    key: "attr1".as_bytes().to_vec(),
                    value: String("attr1_value1".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "attr2".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "attr3".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr1, );
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr2, );
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr3, );
                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "attr2".as_bytes().to_vec(),
                    val : String("attr1_value1".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "attr1".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "attr3".as_bytes().to_vec(),
                    val : Uint64(22),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));
                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft_id,
                };
                let res = selector.validate();
                assert_eq!(selector.token_count(), 5);
                assert_eq!(res,  Ok(()));
                // OrderModule::token_buy_order_create(Origin::signed(bob),token_id, attr2, );


                stack.clear();
                stack.push(Uint8T(f4.clone()));
                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft_id,
                };
                let res = selector.validate();
                assert_eq!(res,  Err("Can not reduce stack to boolean"));

                stack.clear();
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));
                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack: stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft_id,
                };
                let res = selector.validate();
                assert_eq!(res,  Err("Can not reduce stack to single element"));
                
                

                stack.clear();
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));
                stack.push(BoolExp(f3.clone()));
                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack,
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft_id,
                };
                let res = selector.validate();
                assert_eq!(res,  Err("Invalid dfs stack, logic comparison expected"));
                
                
            });
        }
        
        #[test]
        fn token_match_visitor_test() {
            new_test_ext().execute_with(|| {

                fn print_attributes(attr_value: &TokenAttrValType ){
                    match &attr_value{
                        String(x1) => {// Vec<u8>
                            println!("String as => {}", str::from_utf8(&x1).unwrap());
                        },
                        Uint64(x2) => {
                            println!("integer as => {}", x2);
                        },
                    }
                    
                };
                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                let nft_id = NftsModule::get_nft_by_index(0);

                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft_id, "token_1".as_bytes().to_vec());
                let token_id = Erc721Module::token_by_index(0);
                let bob_nfts = NftsModule::get_nfts_owner_vec(&bob);
                let token = NftsModule::get_token(&token_id);
                println!("{:?}", token);
                println!("------------------------Print get_nfts_owner_vec-----------------------", );
                for it in bob_nfts.iter(){
                    println!("nft_id is => {:?}", it);
                };
                println!("------------------------Print get_nfts_owner_vec end-----------------------", );

                let attr1 = Attributes{
                    key: "attr1".as_bytes().to_vec(),
                    value: String("attr1_value1".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "attr2".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "attr3".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr1, );
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr2, );
                OrderModule::set_token_attr(Origin::signed(bob),token_id, attr3, );
                let token_attrs = OrderModule::get_token_attr(token_id);//TokenAttrType
                for (k, v ) in token_attrs.iter(){
                    println!("key => {}", str::from_utf8(&k).unwrap());
                    print_attributes(v);
                };
                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "attr1".as_bytes().to_vec(),
                    val : String("attr1_value1".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "attr2".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "attr3".as_bytes().to_vec(),
                    val : Uint64(22),
                };
                let f5 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "attr3".as_bytes().to_vec(),
                    val : Uint64(10),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));
                println!("------------------------Print stack-----------------------", );
                for s in stack.iter(){
                    match s {
                        BoolExp(ss) => {
                            println!("key => {}", str::from_utf8(&ss.key).unwrap());
                            print_attributes(&ss.val);
                        },
                        Uint8T(t2) => {
                            println!("logic => {:?}", t2);
                        },
                    }
                    
                };
                println!("------------------------Print stack end----------------------- ", );
                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft_id,
                };
                let is_match = _filter_match( &token_attrs, &f2 );
                assert_eq!(is_match, true);
                let is_match = _filter_match( &token_attrs, &f1 );
                assert_eq!(is_match, true);
                let is_match = _filter_match( &token_attrs, &f3 );
                assert_eq!(is_match, true);
                let is_match = _filter_match( &token_attrs, &f5 );
                assert_eq!(is_match, false);
                let is_match = _token_selector_match(&token_attrs, &attr_selector);
                assert_eq!(is_match, true);

                let bind_tokens = OrderModule::_token_match_visitor(bob, &selector) ;
                println!("-----------------------chosen tokens ----------------------- " );
                for &it in bind_tokens.iter(){
                    println!("{:?}", it);
                };
                
            });
        }
        fn print_orderbook(asset: <Test as generic_asset::Trait>::AssetId){
            let b_orderbook = OrderModule::get_orderbook_bid(&asset);
            for (&price, order_vec) in b_orderbook.iter().rev() {
                println!("Price => {}", price);
                for &o in order_vec.iter(){
                    let order = match OrderModule::get_bid_token_order(o){
                        Some(t) => t,
                        None => continue,
                    };
                    println!("{:?}", order);
                }
            };
            println!("----------------- end of bid orderbook ----------------- ");
            let a_orderbook = OrderModule::get_orderbook_ask(&asset);
            for (&price, order_vec) in a_orderbook.iter() {
                println!("Price => {}", price);
                for &o in order_vec.iter(){
                    let order = match OrderModule::get_ask_token_order(o){
                        Some(t) => t,
                        None => continue,
                    };
                    println!("{:?}", order);
                }
            };
            println!("----------------- end of ask orderbook -----------------");
            
        }

        #[test]
        fn order_create_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);

                // let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 250, false);
                // assert_eq!(res, Ok(()) );
                // println!("-----------------After create buy order form alice for asset0@250", );
                // print_orderbook(asset0);

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 200, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@200", );
                
                print_orderbook(asset0);

            });
        }
        #[test]
        fn order_ask_more_fill_bid_less_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());

                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                
                NftsModule::issue_token(Origin::signed(bob), nft1, "bob1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "bob2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "bob3".as_bytes().to_vec());

                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let tk5 = Erc721Module::token_by_index(4);
                let tk6 = Erc721Module::token_by_index(5);
                let tk7 = Erc721Module::token_by_index(6);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 1, // 1 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                // let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 200, false);
                // assert_eq!(res, Ok(()) );
                // println!("-----------------After create sell order form bob for asset0@200", );
                
                // print_orderbook(asset0);

                
                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 250, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@250", );
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 1 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 200, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@200", );
                
                print_orderbook(asset0);

            });
        }
        #[test]
        fn order_ask_less_fill_bid_more_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 2500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );
                
                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 250, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@250", );
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 200, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@200", );
                
                print_orderbook(asset0);

            });
        }
        
        #[test]
        fn order_ask_kill_fill_bid_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );
                
                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 250, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@250", );
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 200, true);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@200", );
                
                print_orderbook(asset0);

            });
        }
        
        #[test]
        fn order_bid_more_fill_ask_less_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f0 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("fantastic".as_bytes().to_vec()),
                };

                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk1 chosen
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f0.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk2 chosen
                print_orderbook(asset0);
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form alice for asset0@150", );// tk3 chosen
                print_orderbook(asset0);
                


                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_buy_order_create(Origin::signed(jack), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form jack for asset0@100", );
                print_orderbook(asset0);
                assert_eq!(Erc721Module::owner_of(&tk1).unwrap(), jack);
                assert_eq!(Erc721Module::owner_of(&tk2).unwrap(), jack);
                

                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);
                assert_eq!(Erc721Module::owner_of(&tk3).unwrap(), alice);
                

            });
        }
        
        #[test]
        fn order_bid_less_fill_ask_more_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f0 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("fantastic".as_bytes().to_vec()),
                };

                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk1 chosen
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f0.clone()));
                stack.push(BoolExp(f2.clone()));

                let stack_0_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk2 chosen
                print_orderbook(asset0);
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form alice for asset0@150", );// tk3 chosen
                print_orderbook(asset0);
                


                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let stack_1_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 1,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_buy_order_create(Origin::signed(jack), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form jack for asset0@100", );
                print_orderbook(asset0);
                assert_eq!(Erc721Module::owner_of(&tk1).unwrap(), jack);
                // assert_eq!(Erc721Module::owner_of(&tk2).unwrap(), jack);
                
                let attr_selector = TokenAttrSelector{
                    max_count: 1,
                    stack:stack_0_or_2.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);
                assert_eq!(Erc721Module::owner_of(&tk3).unwrap(), alice);
                

            });
        }
        
        #[test]
        fn order_bid_kill_fill_ask_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f0 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("fantastic".as_bytes().to_vec()),
                };

                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk1 chosen
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f0.clone()));
                stack.push(BoolExp(f2.clone()));

                let stack_0_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 60, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@60", );// tk2 chosen
                print_orderbook(asset0);
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(alice), selector.clone(), asset0, 150, true);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form alice for asset0@150", );// tk3 chosen
                print_orderbook(asset0);
                


                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f2.clone()));

                let stack_1_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 2,
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_buy_order_create(Origin::signed(jack), selector.clone(), asset0, 70, true);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form jack for asset0@70", );
                print_orderbook(asset0);
                let res = OrderModule::token_buy_order_create(Origin::signed(jack), selector.clone(), asset0, 110, true);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form jack for asset0@110", );
                print_orderbook(asset0);
                assert_eq!(Erc721Module::owner_of(&tk1).unwrap(), jack);
                assert_eq!(Erc721Module::owner_of(&tk2).unwrap(), jack);
                
                let attr_selector = TokenAttrSelector{
                    max_count: 2,
                    stack:stack_0_or_2.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };

                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form alice for asset0@150", );
                print_orderbook(asset0);
                // assert_eq!(Erc721Module::owner_of(&tk3).unwrap(), alice);
                

            });
        }
        
        #[test]
        fn order_ask_cancel_test() {
            new_test_ext().execute_with(|| {

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f0 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("fantastic".as_bytes().to_vec()),
                };

                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@100", );// tk1 chosen
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f0.clone()));
                stack.push(BoolExp(f2.clone()));

                let stack_0_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 5,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector.clone(), asset0, 60, false);
                println!("-----------------After create sell order form bob for asset0@60", );// tk2 chosen
                print_orderbook(asset0);
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };
                let res = OrderModule::token_sell_order_create(Origin::signed(alice), selector.clone(), asset0, 150, true);
                println!("-----------------After create sell order form alice for asset0@150", );// tk3 chosen
                print_orderbook(asset0);

                let ordermap = OrderModule::get_orderbook_ask(&asset0);
                let order1 = ordermap.get(&100).unwrap().get(0).unwrap();
                let order2 = ordermap.get(&60).unwrap().get(0).unwrap();
                let order3 = ordermap.get(&150);
                assert_eq!(order3, None);

                let res = OrderModule::token_sell_order_cancel(Origin::signed(bob), *order1);
                println!("-----------------After cancel sell order form alice for asset0@100", );
                print_orderbook(asset0);
                let res = OrderModule::token_sell_order_cancel(Origin::signed(bob), *order2);
                println!("-----------------After cancel sell order form alice for asset0@60", );
                print_orderbook(asset0);

            });
        }
        
        #[test]
        fn order_bid_cancel_test() {
            new_test_ext().execute_with(|| {
                // let god = 100;
                

                let bob = 0;
                let bobby = 3;
                let alice = 1;  
                let jack = 2;  
                NftsModule::nonfungible_create(Origin::signed(bob), "doggy".as_bytes().to_vec(), 10);
                NftsModule::nonfungible_create(Origin::signed(alice), "catty".as_bytes().to_vec(), 120);
                let nft1 = NftsModule::get_nft_by_index(0);
                let nft2 = NftsModule::get_nft_by_index(1);

                NftsModule::issue_token(Origin::signed(bob), nft1, "token_0".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(bob), nft1, "token_1".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_2".as_bytes().to_vec());
                NftsModule::issue_token(Origin::signed(alice), nft2, "token_3".as_bytes().to_vec());
                let tk1 = Erc721Module::token_by_index(0);
                let tk2 = Erc721Module::token_by_index(1);
                let tk3 = Erc721Module::token_by_index(2);
                let tk4 = Erc721Module::token_by_index(3);

                let attr0 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("fantastic".as_bytes().to_vec()),
                };
                let attr1 = Attributes{
                    key: "prefix".as_bytes().to_vec(),
                    value: String("great".as_bytes().to_vec()),
                };
                let attr2 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(88),
                };
                let attr3 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(20),
                };
                let attr4 = Attributes{
                    key: "age".as_bytes().to_vec(),
                    value: Uint64(50),
                };
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr1.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk1, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(bob),tk2, attr2.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr0.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk3, attr3.clone(), );

                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr4.clone(), );
                let res = OrderModule::set_token_attr(Origin::signed(alice),tk4, attr1.clone(), );

                let mut stack = Vec::<FilterItem>::new();
                let f0 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("fantastic".as_bytes().to_vec()),
                };

                let f1 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "prefix".as_bytes().to_vec(),
                    val : String("great".as_bytes().to_vec()),
                };
                let f2 = BooleanExpression {
                    op: CompareOpcode::TokenCmpEq,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(88),
                };
                let f3 = BooleanExpression {
                    op: CompareOpcode::TokenCmpLt,
                    key: "age".as_bytes().to_vec(),
                    val : Uint64(30),
                };
                let f4 = LogicOpcode::TokenLogicOr;
                let f5 = LogicOpcode::TokenLogicAnd;

                stack.push(Uint8T(f5.clone()));
                stack.push(BoolExp(f1.clone()));
                stack.push(BoolExp(f3.clone()));

                let attr_selector = TokenAttrSelector{
                    max_count: 2, // 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector),
                    nft_type: nft1,
                };

                let asset_issuer = 100;
                use generic_asset::{AssetOptions, PermissionsV1};
                let asset_option = AssetOptions{
                    initial_issuance: 1000000,
                    permissions: PermissionsV1::default(),
                };
                let asset0 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let asset1 = AssetsModule::next_asset_id();
                let res = AssetsModule::create_asset(None, Some(asset_issuer), asset_option.clone());
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, bob, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, alice, 1500);
                assert_eq!(res, Ok(()) );
                let res = AssetsModule::transfer(Origin::signed(asset_issuer), asset0, jack, 1500);
                assert_eq!(res, Ok(()) );
                

                


                let res = OrderModule::token_buy_order_create(Origin::signed(bob), selector.clone(), asset0, 100, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create buy order form bob for asset0@100", );// tk1 chosen
                print_orderbook(asset0);

                stack.clear();
                stack.push(Uint8T(f4.clone()));
                stack.push(BoolExp(f0.clone()));
                stack.push(BoolExp(f3.clone()));

                let stack_0_or_2 = stack.clone();

                let attr_selector = TokenAttrSelector{
                    max_count: 2,// 2 is less than 5
                    stack:stack.clone(),
                };
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft1,
                };
                let selector_nft1 = selector.clone();
                let res = OrderModule::token_buy_order_create(Origin::signed(bob), selector.clone(), asset0, 60, false);
                println!("-----------------After create buy order form bob for asset0@60", );// tk2 chosen
                print_orderbook(asset0);
                let selector = TokenSelector{
                    selector: AttrSelect(attr_selector.clone()),
                    nft_type: nft2,
                };
                let res = OrderModule::token_buy_order_create(Origin::signed(alice), selector.clone(), asset0, 150, true);
                println!("-----------------After create buy order form alice for asset0@150", );// tk3 chosen
                print_orderbook(asset0);
                
                let ordermap = OrderModule::get_orderbook_bid(&asset0);
                let order1 = ordermap.get(&100).unwrap().get(0).unwrap();
                let order2 = ordermap.get(&60).unwrap().get(0).unwrap();
                let order3 = ordermap.get(&150);
                assert_eq!(order3, None);

                let res = OrderModule::token_sell_order_create(Origin::signed(bob), selector_nft1.clone(), asset0, 50, false);
                assert_eq!(res, Ok(()) );
                println!("-----------------After create sell order form bob for asset0@50", );// tk1 chosen
                print_orderbook(asset0);

                let res = OrderModule::token_buy_order_cancel(Origin::signed(bob),*order1);
                println!("-----------------After cancel buy order form alice for asset0@100", );
                print_orderbook(asset0);
                let res = OrderModule::token_buy_order_cancel(Origin::signed(bob),*order2);
                println!("-----------------After cancel buy order form alice for asset0@60", );
                print_orderbook(asset0);
                println!("-----------------After cancel buy order again form alice for asset0@60", );
                let res = OrderModule::token_buy_order_cancel(Origin::signed(bob),*order2);
                assert_eq!(res, Err("order not found"));
                
                

            });
        }
        
}