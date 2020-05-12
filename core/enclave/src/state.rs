//! This module defines operations for each user's state.

use anonify_common::{
    UserAddress, Sha256, Hash256, LockParam, AccessRight,
    kvs::*,
};
use anonify_app_preluder::{CallKind, MAX_MEM_SIZE, Ciphertext, Runtime};
use anonify_runtime::{StateType, State, StateGetter, UpdatedState, into_trait, MemId};
use codec::{Encode, Decode, Input, Output};
use crate::{
    crypto::*,
    kvs::EnclaveDBTx,
    error::{Result, EnclaveError},
    context::{EnclaveContext},
    group_key::GroupKey,
};
use std::{
    vec::Vec,
    io::{Write, Read},
    marker::PhantomData,
    convert::{TryFrom, TryInto},
    collections::HashMap,
};

/// An collection of state transition operations
#[derive(Clone)]
pub struct StateTransService<S: State>{
    ctx: EnclaveContext<S>,
    my_addr: UserAddress,
    updates: Option<Vec<UpdatedState<StateType>>>,
}

impl StateTransService<StateType>
{
    /// Only way to generate StateTransService is given by access right.
    pub fn from_access_right(
        access_right: &AccessRight,
        ctx: &EnclaveContext<StateType>,
    ) -> Result<Self> {
        let my_addr = UserAddress::from_access_right(access_right)?;
        let ctx = ctx.clone();

        Ok(StateTransService::<StateType>{
            ctx,
            my_addr,
            updates: None,
        })
    }

    /// Apply calling function parameters and call name to
    /// state transition functions.
    pub fn apply(
        &mut self,
        kind: CallKind,
    ) -> Result<()> {
        let res = Runtime::new(self.ctx.clone()).call(
            kind,
            self.my_addr,
        )?;

        self.updates = Some(res);

        Ok(())
    }

    /// Return current state's lock parameters of each user.
    // TODO: Consider; is it OK that init_lock_param = H(address||mem_id||zero_sv)
    pub fn create_lock_params(&self) -> Vec<LockParam> {
        self.updates
            .clone()
            .expect("State transitions are not applied.")
            .into_iter()
            .map(|e| {
                let sv = self.ctx.state_value(e.address, e.mem_id);
                UserState::<StateType, Current>::new(e.address, e.mem_id, sv)
                    .lock_param()
            })
            .collect()
    }

    /// Return ciphertexts data which is generates by encrypting updated user's state.
    pub fn create_ciphertexts(&self, geoup_key: &GroupKey) -> Result<Vec<Ciphertext>> {
        self.updates
            .clone()
            .expect("State transitions are not applied.")
            .into_iter()
            .map(|e| {
                let sv = self.ctx.state_value(e.address, e.mem_id);
                UserState::<StateType, Current>::new(e.address, e.mem_id, sv)
                    .update_inner_state(e.state)
                    .into_next()
                    .encrypt(geoup_key)
            })
            .collect()
    }
}

/// Current generation of lock parameter for state.
/// Preventing from race condition of writing ciphertext to blockchain.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Current;

/// Next generation of lock parameter for state.
/// It'll be defined deterministically as `next_lock_param = Hash(address, current_state, current_lock_param)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Next;

/// This struct can be got by decrypting ciphertexts which is stored on blockchain.
/// The group encryption key is shared among all TEE's enclaves.
#[derive(Encode, Decode, Debug, Clone, PartialEq)]
pub struct UserState<S: State, N> {
    address: UserAddress,
    mem_id: MemId,
    state_value: StateValue<S, N>,
}

impl<N> UserState<StateType, N> {
    pub fn inner_state(&self) -> &StateType {
        &self.state_value.inner_state
    }

    pub fn lock_param(&self) -> LockParam {
        self.state_value.lock_param
    }
}

/// Operations of user state before sending a transaction or after fetching as a ciphertext
impl UserState<StateType, Current> {
    pub fn new(
        address: UserAddress,
        mem_id: MemId,
        state_value: StateValue<StateType, Current>,
    ) -> Self {
        UserState {
            address,
            mem_id,
            state_value,
        }
    }

    /// Decrypt Ciphertext which was stored in a shared ledger.
    pub fn decrypt(cipheriv: &Ciphertext, key: &mut GroupKey) -> Result<Option<Self>> {
        match key.decrypt(cipheriv)? {
            Some(plaintext) => {
                UserState::decode(&mut &plaintext[..])
                    .map(|p| Some(p))
                    .map_err(Into::into)
            }
            None => Ok(None)
        }
    }

    pub fn mem_id(&self) -> MemId {
        self.mem_id
    }

    /// Get in-memory database key.
    pub fn address(&self) -> UserAddress {
        self.address
    }

    pub fn into_sv(self) -> StateValue<StateType, Current> {
        self.state_value
    }

    pub fn update_inner_state(self, update: StateType) -> Self {
        let state_value = StateValue::new(update, self.lock_param());

        UserState {
            address: self.address,
            mem_id: self.mem_id,
            state_value,
        }
    }

    /// Convert into userstate of next lock parameter generation.
    /// This is called when it's ready to encrypt state which will be sent to outside.
    /// Basically, this allows us to prevent from data collisions in the public shared database.
    pub fn into_next(self) -> UserState<StateType, Next> {
        let next_lock_param = self.next_lock_param();
        let inner_state = self.state_value.inner_state;
        let state_value = StateValue::new(inner_state, next_lock_param);

        UserState {
            address: self.address,
            mem_id: self.mem_id,
            state_value,
        }
    }

    /// Generate lock parameters of next generations based on current user's state.
    fn next_lock_param(&self) -> LockParam {
        let next_lock_param = self.hash();
        next_lock_param.into()
    }

    /// Compute hash digest of current user state.
    fn hash(&self) -> Sha256 {
        let inp = self.encode();
        Sha256::hash(&inp)
    }
}

/// A UserState which has a lock parameter of next generation has only encrypt function
/// so that it allows us to do nothing other than generating ciphertexts which will be sent
/// to blockchain node.
impl UserState<StateType, Next> {
    pub fn encrypt(self, key: &GroupKey) -> Result<Ciphertext> {
        // Add padding to fix the ciphertext size of all state types.
        // The padding works for fixing the ciphertext size so that
        // other people cannot distinguish what state is encrypted based on the size.
        fn append_padding(buf: &mut Vec<u8>) {
            let padding_size = MAX_MEM_SIZE - buf.len();
            let mut padding = vec![0u8; padding_size];
            buf.extend_from_slice(&mut padding);
        }

        let mut buf = self.encode();
        append_padding(&mut buf);
        key.encrypt(buf).map_err(Into::into)
    }
}

/// State value per each user's state.
/// inner_state depends on the state of your application on anonify system.
/// LockParam is used to avoid data collisions when TEEs send transactions to blockchain.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Default)]
pub struct StateValue<S: State, N> {
    pub inner_state: S,
    pub lock_param: LockParam,
    _marker: PhantomData<N>,
}

impl<N> StateValue<StateType, N> {
    pub fn new(inner_state: StateType, lock_param: LockParam) -> Self {
        StateValue {
            inner_state,
            lock_param,
            _marker: PhantomData,
        }
    }

    /// Get inner state and lock_param from database value.
    pub fn from_dbvalue(db_value: DBValue) -> Result<Self> {
        let mut state = Default::default();
        let mut lock_param = Default::default();

        if db_value != Default::default() {
            let reader = db_value.into_vec();
            state = StateType::read_le(&mut &reader[..])?;
            lock_param = LockParam::read(&mut &reader[..])?;
        }

        Ok(StateValue::new(state, lock_param))
    }

    pub fn write<W: Write + Output>(&self, writer: &mut W) -> Result<()> {
        self.inner_state.write_le(writer);
        self.lock_param.write(writer)?;

        Ok(())
    }

    pub fn write_with_update<W: Write + Output>(
        &self,
        writer: &mut W,
        update: impl State,
    ) -> Result<()> {
        update.write_le(writer);
        self.lock_param.write(writer)?;

        Ok(())
    }

    pub fn read<R: Read + Input>(reader: &mut R) -> Result<Self> {
        let inner_state = StateType::read_le(reader)?;
        let lock_param = LockParam::read(reader)?;

        Ok(StateValue::new(inner_state, lock_param))
    }

    pub fn as_inner_state(&self) -> &StateType {
        &self.inner_state
    }

    pub fn into_inner_state(self) -> StateType {
        self.inner_state
    }

    pub fn lock_param(&self) -> &LockParam {
        &self.lock_param
    }
}

#[cfg(debug_assertions)]
pub mod tests {
    use super::*;
    use anonify_runtime::StateType;
    use ed25519_dalek::{SecretKey, PublicKey, Keypair, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH};

    const SECRET_KEY_BYTES: [u8; SECRET_KEY_LENGTH] = [
        062, 070, 027, 163, 092, 182, 011, 003,
        077, 234, 098, 004, 011, 127, 079, 228,
        243, 187, 150, 073, 201, 137, 076, 022,
        085, 251, 152, 002, 241, 042, 072, 054, ];

    const PUBLIC_KEY_BYTES: [u8; PUBLIC_KEY_LENGTH] = [
        130, 039, 155, 015, 062, 076, 188, 063,
        124, 122, 026, 251, 233, 253, 225, 220,
        014, 041, 166, 120, 108, 035, 254, 077,
        160, 083, 172, 058, 219, 042, 086, 120, ];

    pub fn test_read_write() {
        // let secret = SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap();
        // let public = PublicKey::from_bytes(&PUBLIC_KEY_BYTES).unwrap();
        // let keypair = Keypair { secret, public };

        // let mut buf = vec![];
        // StateType::new(100).write_le(&mut buf);

        // let sig = keypair.sign(&buf);
        // let user_address = UserAddress::from_sig(&buf, &sig, &public).unwrap();

        // let state = UserState::<StateType, Next>::init(user_address, StateType::new(100)).unwrap();
        // let state_vec = state.try_into_vec().unwrap();
        // let res = UserState::read(&state_vec[..]).unwrap();

        // assert_eq!(state, res);
    }
}
