use anonify_config::{IAS_ROOT_CERT, LOCAL_PATH_SECRETS_DIR};
use anonify_io_types::*;
use anyhow::{anyhow, Result};
use codec::{Decode, Encode};
use frame_common::{crypto::Sha256, state_types::StateType};
use frame_enclave::EnclaveEngine;
use frame_runtime::traits::*;
use frame_treekem::{handshake::HandshakeParams, StorePathSecrets};

/// A add handshake Sender
#[derive(Debug, Clone)]
pub struct JoinGroupSender;

impl EnclaveEngine for JoinGroupSender {
    type EI = input::CallJoinGroup;
    type EO = output::ReturnJoinGroup;

    fn handle<R, C>(
        _ecall_input: Self::EI,
        enclave_context: &C,
        _max_mem_size: usize,
    ) -> Result<Self::EO>
    where
        R: RuntimeExecutor<C, S = StateType>,
        C: ContextOps<S = StateType> + Clone,
    {
        let attested_report = enclave_context.quote()?.remote_attestation(
            enclave_context.ias_url(),
            enclave_context.sub_key(),
            IAS_ROOT_CERT.to_vec(),
        )?;

        let mrenclave_ver = enclave_context.mrenclave_ver();
        let group_key = &*enclave_context.read_group_key();
        let (handshake, path_secret) = group_key.create_handshake()?;
        let epoch = handshake.prior_epoch();
        let id = handshake.hash();
        let export_path_secret = path_secret.clone().try_into_exporting(epoch, id.as_ref())?;
        let store_path_secrets = StorePathSecrets::new(LOCAL_PATH_SECRETS_DIR);
        store_path_secrets.save_to_local_filesystem(&export_path_secret)?;
        let export_handshake = handshake.clone().into_export();

        #[cfg(feature = "backup-enable")]
        enclave_context.backup_path_secret_to_key_vault(
            path_secret.as_bytes().to_vec(),
            epoch,
            handshake.roster_idx(),
            id.as_ref().to_vec(),
        )?;

        Ok(output::ReturnJoinGroup::new(
            attested_report.report().to_vec(),
            attested_report.report_sig().to_vec(),
            export_handshake.encode(),
            mrenclave_ver,
            export_handshake.roster_idx(),
        ))
    }
}

/// A update handshake sender
#[derive(Debug, Clone)]
pub struct HandshakeSender;

impl EnclaveEngine for HandshakeSender {
    type EI = input::CallHandshake;
    type EO = output::ReturnHandshake;

    fn handle<R, C>(
        _ecall_input: Self::EI,
        enclave_context: &C,
        _max_mem_size: usize,
    ) -> Result<Self::EO>
    where
        R: RuntimeExecutor<C, S = StateType>,
        C: ContextOps<S = StateType> + Clone,
    {
        let group_key = &*enclave_context.read_group_key();
        let (handshake, path_secret) = group_key.create_handshake()?;
        let epoch = handshake.prior_epoch();
        let id = handshake.hash();
        let export_path_secret = path_secret.clone().try_into_exporting(epoch, id.as_ref())?;
        let store_path_secrets = StorePathSecrets::new(LOCAL_PATH_SECRETS_DIR);
        store_path_secrets.save_to_local_filesystem(&export_path_secret)?;
        let export_handshake = handshake.clone().into_export();

        #[cfg(feature = "backup-enable")]
        enclave_context.backup_path_secret_to_key_vault(
            path_secret.as_bytes().to_vec(),
            epoch,
            handshake.roster_idx(),
            id.as_ref().to_vec(),
        )?;

        let roster_idx = export_handshake.roster_idx();
        let msg = Sha256::hash_with_u32(&export_handshake.encode(), roster_idx);
        let sig = enclave_context.sign(msg.as_bytes())?;
        let enclave_sig = sig.0;
        let recovery_id = sig.1;

        Ok(output::ReturnHandshake::new(
            export_handshake,
            enclave_sig,
            recovery_id,
            roster_idx,
        ))
    }
}

/// A handshake receiver
#[derive(Debug, Clone)]
pub struct HandshakeReceiver;

impl EnclaveEngine for HandshakeReceiver {
    type EI = input::InsertHandshake;
    type EO = output::Empty;

    fn handle<R, C>(
        ecall_input: Self::EI,
        enclave_context: &C,
        _max_mem_size: usize,
    ) -> Result<Self::EO>
    where
        R: RuntimeExecutor<C, S = StateType>,
        C: ContextOps<S = StateType> + Clone,
    {
        let group_key = &mut *enclave_context.write_group_key();
        let handshake = HandshakeParams::decode(&mut &ecall_input.handshake().handshake()[..])
            .map_err(|_| anyhow!("HandshakeParams::decode Error"))?;

        let spid = enclave_context.spid();
        let ias_url = enclave_context.ias_url();
        let sub_key = enclave_context.sub_key();
        let server_address = enclave_context.server_address();

        group_key.process_handshake(&handshake, spid, ias_url, sub_key, server_address)?;

        Ok(output::Empty::default())
    }
}
