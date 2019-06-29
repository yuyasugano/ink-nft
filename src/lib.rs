#![cfg_attr(not(any(test, feature = "std")), no_std)]

use ink_core::{
    env::{self, AccountId},
    storage,
};
use ink_lang::contract;
use parity_codec::{Decode, Encode};

contract! {

    /// Storage values of the contract
    struct NFToken {
        /// Owner of contract
        owner: storage::Value<AccountId>,
        /// Total tokens minted
        total_minted: storage::Value<u64>,
        /// Mapping: token_id(u64) -> owner (AccountID)
        id_to_owner: storage::HashMap<u64, AccountId>,
        /// Mapping: owner(AccountID) => tokenCount (u64)
        owner_to_token_count: storage::HashMap<AccountId, u64>,
        /// Mapping: token_id(u64) to account(AccountId)
        approvals: storage::HashMap<u64, AccountId>,
    }

    /// compulsary deploy method
    impl Deploy for NFToken {
        /// Initializes our initial total minted value to 0.
        fn deploy(&mut self, init_value: u64) {
            self.total_minted.set(0);
            // set ownership of contract
            self.owner.set(env.caller());
            // mint initial tokens
            if init_value > 0 {
                self.mint_impl(env.caller(), init_value);
            }
        }
    }

    /// Events
    event EventMint { owner: AccountId, value: u64 }
    event EventTransfer { from: AccountId, to: AccountId, token_id: u64 }
    event EventApproval { owner: AccountId, spender: AccountId, token_id: u64, approved: bool }

    /// Public methods
    impl NFToken {

        /// Returns whether an account is approved to send a token
        pub(external) fn is_approved(&self, token_id: u64, approved: AccountId) -> bool {
            let approval = self.approvals.get(&token_id); // Borrowing &token_id reference
            // AccountId returns option
            if let None = approval {
                return false;
            }
            if *approval.unwrap() == approved {
                return true;
            }
            false
        }

        /// Return the total amount of tokens ever minted
        pub(external) fn total_minted(&self) -> u64 {
            let total_minted = *self.total_minted;
            total_minted
        }

        /// Return the balance of the given address
        pub(external) fn balance_of(&self, owner: AccountId) -> u64 {
            let balance = *self.owner_to_token_count.get(&owner).unwrap_or(&0);
            balance
        }

        /// Transfers a token_id to a specified address from the caller
        pub(external) fn transfer(&mut self, to: AccountId, token_id: u64) -> bool {
            // carry out the actual transfer
            if self.transfer_impl(env.caller(), to, token_id) == true {
                env.emit(EventTransfer { from: env.caller(), to: to, token_id: token_id });
                return true;
            }
            false
        }

        /// Transfers a token_id from a specified address to another specified address
        pub(external) fn transfer_from(&mut self, to: AccountId, token_id: u64) -> bool {
            // make the transfer immediately if caller is the owner
            if self.is_token_owner(&env.caller(), token_id) { // &env.caller() gives a reference
                let result = self.transfer_impl(env.caller(), to, token_id);
                if result == true {
                    env.emit(EventTransfer { from: env.caller(), to: to, token_id: token_id });
                }
                return result;

            // not owner: check if caller is approved to move the token
            } else {
                let approval = self.approvals.get(&token_id);
                if let None = approval {
                    return false;
                }

                // carry out transfer if caller is approved
                if *approval.unwrap() == env.caller() {
                    // carry out the actual transfer
                    let result = self.transfer_impl(env.caller(), to, token_id);
                    if result == true {
                        env.emit(EventTransfer { from: env.caller(), to: to, token_id: token_id });
                    }
                    return result;
                } else {
                    return false;
                }
            }
        }
        
        /// Mints a specified amount of new tokens to a given address
        pub(external) fn mint(&mut self, to: AccountId, value: u64) -> bool {
            if env.caller() != *self.owner {
                return false;
            }

            // carry out the actual minting
            if self.mint_impl(to, value) == true {
                env.emit(EventMint { owner: to, value: value });
                return true;
            }
            false
        }

        /// Approves or disapproves an Account to send token on behalf of an owner
        pub(external) fn approval(&mut self, to: AccountId, token_id: u64, approved: bool) -> bool {
            // return if caller is not the token owner
            let token_owner = self.id_to_owner.get(&token_id);
            if let None = token_owner {
                return false;
            }

            let token_owner = *token_owner.unwrap();
            if token_owner != env.caller() {
                return false;
            }

            let approvals = self.approvals.get(&token_id);

            // insert approval if
            if let None = approvals {
                if approved == true {
                    self.approvals.insert(token_id, to);
                } else {
                    return false;
                }

            } else {
                let existing = *approvals.unwrap();

                // remove existing owner if disapproving
                // disapprove is possible
                if existing == to && approved == false {
                    self.approvals.remove(&token_id);
                }

                // overwrite or insert if approving is true
                if approved == true {
                    self.approvals.insert(token_id, to);
                }
            }

            env.emit(EventApproval { owner: env.caller(), spender: to, token_id: token_id, approved: approved });
            true
        }
    }


    /// Private methods
    impl NFToken {

        /// 
        fn is_token_owner(&self, of: &AccountId, token_id: u64) -> bool {
            let owner = self.id_to_owner.get(&token_id);
            if let None = owner {
                return false;
            }
            let owner = *owner.unwrap();
            if owner != *of {
                return false;
            }
            true
        }

        /// Transfers token from a specified address to another address
        fn transfer_impl(&mut self, from: AccountId, to: AccountId, token_id: u64) -> bool {
            if !self.is_token_owner(&from, token_id) {
                return false;
            }

            self.id_to_owner.insert(token_id, to);

            // update owner token counts
            let from_owner_count = *self.owner_to_token_count.get(&from).unwrap_or(&0);
            let to_owner_count = *self.owner_to_token_count.get(&to).unwrap_or(&0);

            self.owner_to_token_count.insert(from, from_owner_count - 1);
            self.owner_to_token_count.insert(to, to_owner_count + 1);
            true
        }

        /// minting of new tokens implementation
        fn mint_impl(&mut self, receiver: AccountId, value: u64) -> bool {

            let start_id = *self.total_minted + 1;
            let stop_id = *self.total_minted + value;

            // loop through new tokens being minted
            for token_id in start_id..stop_id {
                self.id_to_owner.insert(token_id, receiver);
            }

            // update total supply of owner
            let from_owner_count = *self.owner_to_token_count.get(&self.owner).unwrap_or(&0);
            self.owner_to_token_count.insert(*self.owner, from_owner_count + value);

            // update total supply
            self.total_minted += value;
            true
        }

    }
}

#[cfg(all(test, feature = "test-env"))]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn it_works() {

        // deploying and miting initial tokens
        let mut _nftoken = NFToken::deploy_mock(100);
        let alice = AccountId::try_from([0x0; 32]).unwrap();
        let bob = AccountId::try_from([0x1; 32]).unwrap();
        let charlie = AccountId::try_from([0x2; 32]).unwrap();
        let dave = AccountId::try_from([0x3; 32]).unwrap();

        let total_minted = _nftoken.total_minted();
        assert_eq!(total_minted, 100);

        // transferring token_id from alice to bob
        _nftoken.transfer(bob, 1);

        let alice_balance = _nftoken.balance_of(alice);
        let mut bob_balance = _nftoken.balance_of(bob);

        assert_eq!(alice_balance, 99);
        assert_eq!(bob_balance, 1);

        // approve charlie to send token_id 2 from alice's account
        _nftoken.approval(charlie, 2, true);
        assert_eq!(_nftoken.is_approved(2, charlie), true);

        // overwrite charlie's approval with dave's approval
        _nftoken.approval(dave, 2, true);
        assert_eq!(_nftoken.is_approved(2, dave), true);

        // remove dave from approvals
        _nftoken.approval(dave, 2, false);
        assert_eq!(_nftoken.is_approved(2, dave), false);

        // transfer_from function: caller is token owner
        _nftoken.approval(charlie, 3, true);
        assert_eq!(_nftoken.is_approved(3, charlie), true);

        _nftoken.transfer_from(bob, 3);
        bob_balance = _nftoken.balance_of(bob);

        assert_eq!(bob_balance, 2);
    }
}
