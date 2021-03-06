use frame_common::{state_types::StateType, EcallInput, EcallOutput};
use frame_runtime::{ConfigGetter, ContextOps, RuntimeExecutor};
use serde::{de::DeserializeOwned, Serialize};

pub trait EnclaveEngine: Sized + Default {
    type EI: EcallInput + DeserializeOwned + Default;
    type EO: EcallOutput + Serialize + Default;

    fn decrypt<C>(_ciphertext: Self::EI, _enclave_context: &C) -> anyhow::Result<Self>
    where
        C: ContextOps<S = StateType> + Clone,
    {
        Ok(Self::default())
    }

    /// Evaluate policies like authentication and idempotency
    fn eval_policy(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// If the module have a state transition runtime, use this handler.
    fn handle<R, C>(self, _enclave_context: &C, _max_mem_size: usize) -> anyhow::Result<Self::EO>
    where
        R: RuntimeExecutor<C, S = StateType>,
        C: ContextOps<S = StateType> + Clone,
    {
        Ok(Self::EO::default())
    }

    /// If the module doesn't have a state transition runtime, use this handler.
    fn handle_without_runtime<C>(_enclave_context: &C) -> anyhow::Result<Self::EO>
    where
        C: ConfigGetter,
    {
        Ok(Self::EO::default())
    }
}
