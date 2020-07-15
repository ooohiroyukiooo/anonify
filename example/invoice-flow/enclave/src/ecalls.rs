use std::{
    vec::Vec,
    ptr,
};
use anonify_types::*;
use anonify_common::{
    plugin_types::*,
    commands::*,
};
use anonify_enclave::{
    config::{IAS_URL, TEST_SUB_KEY},
    context::EnclaveContext,
};
use invoice_state_transition::{CIPHERTEXT_SIZE, MAX_MEM_SIZE, Runtime};
use crate::ENCLAVE_CONTEXT;
use anonify_enclave::bridges::inner_ecalls::*;
use anonify_ecalls::register_ecall;
use anyhow::anyhow;
use codec::Encode;

register_ecall!(
    &*ENCLAVE_CONTEXT,
    MAX_MEM_SIZE,
    Runtime<EnclaveContext>,
    EnclaveContext,
    (ENCRYPT_INSTRUCTION_CMD, input::Instruction, output::Instruction),
    // Insert a ciphertext in event logs from blockchain nodes into enclave's memory database.
    (INSERT_CIPHERTEXT_CMD, input::InsertCiphertext, output::ReturnUpdatedState),
);

/// Insert handshake received from blockchain nodes into enclave.
#[no_mangle]
pub unsafe extern "C" fn ecall_insert_handshake(
    handshake: *mut u8,
    handshake_len: usize,
) -> EnclaveStatus {
    if let Err(e) = inner_ecall_insert_handshake(
        handshake,
        handshake_len,
        &*ENCLAVE_CONTEXT,
    ) {
        println!("Error (ecall_insert_handshake): {}", e);
        return EnclaveStatus::error();
    }

    EnclaveStatus::success()
}

/// Get current state of the user represented the given public key from enclave memory database.
#[no_mangle]
pub unsafe extern "C" fn ecall_get_state(
    sig: &RawSig,
    pubkey: &RawPubkey,
    challenge: &RawChallenge, // 32 bytes randomness for avoiding replay attacks.
    mem_id: u32,
    state: &mut EnclaveState,
) -> EnclaveStatus {
    if let Err(e) = inner_ecall_get_state(
        sig,
        pubkey,
        challenge,
        mem_id,
        state,
        &*ENCLAVE_CONTEXT,
    ) {
        println!("Error (ecall_get_state): {}", e);
        return EnclaveStatus::error();
    }

    EnclaveStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn ecall_join_group(
    raw_join_group_tx: &mut RawJoinGroupTx,
) -> EnclaveStatus {
    if let Err(e) = inner_ecall_join_group(
        raw_join_group_tx,
        &*ENCLAVE_CONTEXT,
        IAS_URL,
        TEST_SUB_KEY,
    ) {
        println!("Error (ecall_join_group): {}", e);
        return EnclaveStatus::error();
    }

    EnclaveStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn ecall_handshake(
    raw_handshake_tx: &mut RawHandshakeTx,
) -> EnclaveStatus {
    if let Err(e) = inner_ecall_handshake(
        raw_handshake_tx,
        &*ENCLAVE_CONTEXT,
    ) {
        println!("Error (ecall_handshake): {}", e);
        return EnclaveStatus::error();
    }

    EnclaveStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn ecall_register_notification(
    sig: &RawSig,
    pubkey: &RawPubkey,
    challenge: &RawChallenge,
) -> EnclaveStatus {
    if let Err(e) = inner_ecall_register_notification(
        sig,
        pubkey,
        challenge,
        &*ENCLAVE_CONTEXT,
    ) {
        println!("Error (ecall_register_notification): {}", e);
        return EnclaveStatus::error();
    }

    EnclaveStatus::success()
}