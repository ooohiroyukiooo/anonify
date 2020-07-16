use sgx_types::*;
use anonify_types::{RawHandshakeTx, EnclaveStatus};
use anonify_common::{
    crypto::AccessRight,
    traits::*,
    state_types::UpdatedState,
    plugin_types::*,
    commands::*,
};
use anonify_bc_connector::{
    eventdb::InnerEnclaveLog,
    utils::StateInfo,
    error::{HostError, Result},
};
use log::debug;
use codec::{Encode, Decode};
use crate::auto_ffi::*;
use crate::constants::OUTPUT_MAX_LEN;

extern "C" {
    fn ecall_entry_point(
        eid: sgx_enclave_id_t,
        retval: *mut EnclaveStatus,
        cmd: u32,
        in_buf: *mut u8,
        in_len: usize,
        out_buf: *mut u8,
        out_max: usize,
        out_len: &mut usize,
    ) -> sgx_status_t;
}

pub struct EnclaveConnector{
    eid: sgx_enclave_id_t,
    output_max_len: usize,
}

impl EnclaveConnector {
    pub fn new(eid: sgx_enclave_id_t, output_max_len: usize) -> Self {
        EnclaveConnector {
            eid,
            output_max_len,
        }
    }

    pub fn invoke_ecall<E, D>(&self, cmd: u32, input: E) -> Result<D>
    where
        E: Encode + EcallInput,
        D: Decode + EcallOutput,
    {
        let input_payload = input.encode();
        let result = self.inner_invoke_ecall(cmd, input_payload)?;
        let response = D::decode(&mut &result[..])?;

        Ok(response)
    }

    fn inner_invoke_ecall(&self, cmd: u32, mut input: Vec<u8>) -> Result<Vec<u8>> {
        let input_ptr = input.as_mut_ptr();
        let input_len = input.len();
        let output_max = self.output_max_len;
        let mut output_len = output_max;
        let mut output_buf = Vec::with_capacity(output_max);
        let output_ptr = output_buf.as_mut_ptr();

        let mut ret = EnclaveStatus::default();

        let status = unsafe {
            ecall_entry_point(
                self.eid,
                &mut ret,
                cmd,
                input_ptr,
                input_len,
                output_ptr,
                output_max,
                &mut output_len,
            )
        };

        if status != sgx_status_t::SGX_SUCCESS {
            return Err(HostError::Sgx { status, function: "ecall_entry_point", /*cmd*/ }.into());
        }
        if ret.is_err() {
            return Err(HostError::Enclave { status: ret, function: "ecall_entry_point", /*cmd*/ }.into());
        }
        assert!(output_len < output_max);

        unsafe { output_buf.set_len(output_len); }

        Ok(output_buf)
    }
}

pub(crate) fn encrypt_instruction<S, C>(
    eid: sgx_enclave_id_t,
    access_right: AccessRight,
    state_info: StateInfo<'_, S, C>,
) -> Result<output::Instruction>
where
    S: State,
    C: CallNameConverter,
{
    let input = state_info.crate_input(access_right);
    EnclaveConnector::new(eid, OUTPUT_MAX_LEN)
        .invoke_ecall::<input::Instruction, output::Instruction>(ENCRYPT_INSTRUCTION_CMD, input)
}

pub(crate) fn insert_logs<S: State>(
    eid: sgx_enclave_id_t,
    enclave_log: InnerEnclaveLog,
) -> Result<Option<Vec<UpdatedState<S>>>> {
    if enclave_log.ciphertexts.len() != 0 && enclave_log.handshakes.len() == 0 {
        insert_ciphertexts(eid, enclave_log)
    } else if enclave_log.ciphertexts.len() == 0 && enclave_log.handshakes.len() != 0 {
        // The size of handshake cannot be calculated in this host directory,
        // so the ecall_insert_handshake function is repeatedly called over the number of fetched handshakes.
        for handshake in enclave_log.handshakes {
            insert_handshake(eid, handshake)?;
        }

        Ok(None)
    } else {
        debug!("No logs to insert into the enclave.");
        Ok(None)
    }
}

/// Insert event logs from blockchain nodes into enclave memory database.
fn insert_ciphertexts<S: State>(
    eid: sgx_enclave_id_t,
    enclave_log: InnerEnclaveLog,
) -> Result<Option<Vec<UpdatedState<S>>>> {
    let conn = EnclaveConnector::new(eid, OUTPUT_MAX_LEN);
    let mut acc = vec![];

    for update in enclave_log
        .into_input_iter()
        .map(move |inp|
            conn.invoke_ecall::<input::InsertCiphertext, output::ReturnUpdatedState>(INSERT_CIPHERTEXT_CMD, inp)
        )
    {
        if let Some(upd_type) = update?.updated_state {
            let upd_trait = UpdatedState::<S>::from_state_type(upd_type)?;
            acc.push(upd_trait);
        }
    }

    if acc.is_empty() {
        return Ok(None);
    } else {
        return Ok(Some(acc));
    }
}

fn insert_handshake(
    eid: sgx_enclave_id_t,
    handshake: Vec<u8>,
) -> Result<()> {
    let input = input::InsertHandshake::new(handshake);
    EnclaveConnector::new(eid, OUTPUT_MAX_LEN)
        .invoke_ecall::<input::InsertHandshake, output::Empty>(INSERT_HANDSHAKE_CMD, input)?;

    Ok(())
}

/// Get state only if the signature verification returns true.
pub(crate) fn get_state_from_enclave<M: MemNameConverter>(
    eid: sgx_enclave_id_t,
    access_right: AccessRight,
    mem_name: &str,
) -> Result<Vec<u8>>
{
    let mem_id = M::as_id(mem_name);
    let input = input::GetState::new(access_right, mem_id);

    let state = EnclaveConnector::new(eid, OUTPUT_MAX_LEN)
        .invoke_ecall::<input::GetState, output::ReturnState>(GET_STATE_CMD, input)?;

    Ok(state.into_vec())
}

pub(crate) fn join_group(eid: sgx_enclave_id_t) -> Result<output::ReturnJoinGroup> {
    let input = input::CallJoinGroup::default();
    EnclaveConnector::new(eid, OUTPUT_MAX_LEN)
        .invoke_ecall::<input::CallJoinGroup, output::ReturnJoinGroup>(JOIN_GROUP_CMD, input)
}

/// Handshake to other group members to update the group key
pub(crate) fn handshake(
    eid: sgx_enclave_id_t,
) -> Result<output::ReturnHandshake> {
    let input = input::CallHandshake::default();
    EnclaveConnector::new(eid, OUTPUT_MAX_LEN)
        .invoke_ecall::<input::CallHandshake, output::ReturnHandshake>(HANDSHAKE_CMD, input)
}

pub(crate) fn register_notification(
    eid: sgx_enclave_id_t,
    access_right: AccessRight,
) -> Result<()> {
    let mut rt = EnclaveStatus::default();

    let status = unsafe {
        ecall_register_notification(
            eid,
            &mut rt,
            access_right.sig().to_bytes().as_ptr() as _,
            access_right.pubkey().to_bytes().as_ptr() as _,
            access_right.challenge().as_ptr() as _,
        )
    };

    if status != sgx_status_t::SGX_SUCCESS {
        return Err(HostError::Sgx { status, function: "ecall_register_notification" }.into());
    }
    if rt.is_err() {
        return Err(HostError::Enclave { status: rt, function: "ecall_register_notification" }.into());
    }

    Ok(())
}
