use super::{CryptoRng, SHA256_OUTPUT_LEN};
use crate::local_ring::hmac::{Context, Key, HMAC_SHA256};
use crate::localstd::vec::Vec;
use crate::serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(crate = "crate::serde")]
pub struct HmacKey(Vec<u8>);

impl HmacKey {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[..]
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }

    pub fn new_from_random<R: CryptoRng>(csprng: &mut R) -> HmacKey {
        let mut buf = [0u8; SHA256_OUTPUT_LEN];
        csprng.fill_bytes(&mut buf);
        HmacKey(buf.to_vec())
    }

    #[allow(dead_code)]
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        let signing_key = Key::new(HMAC_SHA256, &self.0);
        let mut ctx = Context::with_key(&signing_key);
        ctx.update(&msg);
        ctx.sign().as_ref().to_vec()
    }
}

impl From<[u8; SHA256_OUTPUT_LEN]> for HmacKey {
    fn from(array: [u8; SHA256_OUTPUT_LEN]) -> Self {
        HmacKey(array.to_vec())
    }
}

impl From<Vec<u8>> for HmacKey {
    fn from(vec: Vec<u8>) -> Self {
        HmacKey(vec)
    }
}

impl From<&[u8]> for HmacKey {
    fn from(bytes: &[u8]) -> Self {
        HmacKey(bytes.to_vec())
    }
}
