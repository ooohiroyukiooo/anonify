use super::{hkdf, hmac::HmacKey};
use crate::base64;
use crate::bincode;
use crate::local_anyhow::{anyhow, Result};
use crate::local_secp256k1::{Error, PublicKey, PublicKeyFormat, SecretKey};
use crate::localstd::{boxed::Box, fmt, vec::Vec};
use crate::serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use frame_common::crypto::rand_assign;

const SECRET_KEY_SIZE: usize = 32;
const COMPRESSED_PUBLIC_KEY_SIZE: usize = 33;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DhPrivateKey(SecretKey);

impl DhPrivateKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let secret_key = SecretKey::parse_slice(bytes).map_err(|e| anyhow!("error: {:?}", e))?;

        Ok(DhPrivateKey(secret_key))
    }

    pub fn from_random() -> Result<Self> {
        let secret = loop {
            let mut ret = [0u8; SECRET_KEY_SIZE];
            rand_assign(&mut ret)?;

            if let Ok(key) = SecretKey::parse(&ret) {
                break key;
            }
        };

        Ok(DhPrivateKey(secret))
    }
}

impl Serialize for DhPrivateKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // only used bincode so it's not human readable
        serializer.serialize_bytes(&self.0.serialize())
    }
}

impl<'de> Deserialize<'de> for DhPrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct DhPrivateKeyVisitor;

        impl<'de> de::Visitor<'de> for DhPrivateKeyVisitor {
            type Value = DhPrivateKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a dh private key bytes")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let pk = SecretKey::parse_slice(value)
                    .map_err(|_e| E::custom(Error::InvalidSecretKey))?;

                Ok(DhPrivateKey(pk))
            }
        }

        deserializer.deserialize_bytes(DhPrivateKeyVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhPubKey(PublicKey);

impl Serialize for DhPubKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.serialize())
    }
}

impl<'de> Deserialize<'de> for DhPubKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct DhPubKeyVisitor;

        impl<'de> de::Visitor<'de> for DhPubKeyVisitor {
            type Value = DhPubKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a bytestring of either 33 (compressed), 64 (raw), or 65 bytes in length",
                )
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let key_format = match value.len() {
                    33 => PublicKeyFormat::Compressed,
                    64 => PublicKeyFormat::Raw,
                    65 => PublicKeyFormat::Full,
                    _ => return Err(E::custom(Error::InvalidInputLength)),
                };
                let pk = PublicKey::parse_slice(value, Some(key_format))
                    .map_err(|_e| E::custom(Error::InvalidPublicKey))?;

                Ok(DhPubKey(pk))
            }
        }

        deserializer.deserialize_bytes(DhPubKeyVisitor)
    }
}

impl Default for DhPubKey {
    fn default() -> Self {
        let secret_key = SecretKey::default();
        DhPubKey(PublicKey::from_secret_key(&secret_key))
    }
}

impl DhPubKey {
    pub fn from_private_key(private_key: &DhPrivateKey) -> Self {
        DhPubKey(PublicKey::from_secret_key(&private_key.0))
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap() // must not fail
    }

    pub fn decode(bytes: &[u8]) -> crate::localstd::result::Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize(bytes)
    }
}

pub fn encapsulate(ephemeral_privkey: &DhPrivateKey, pubkey: &DhPubKey) -> Result<[u8; 32]> {
    let shared_point = diffie_hellman(ephemeral_privkey, pubkey)?;
    let ephemeral_pubkey = PublicKey::from_secret_key(&ephemeral_privkey.0);

    gen_out_buf(&ephemeral_pubkey, &shared_point)
}

pub fn decapsulate(privkey: &DhPrivateKey, ephemeral_pubkey: &DhPubKey) -> Result<[u8; 32]> {
    let shared_point = diffie_hellman(privkey, ephemeral_pubkey)?;

    gen_out_buf(&ephemeral_pubkey.0, &shared_point)
}

fn diffie_hellman(privkey: &DhPrivateKey, pubkey: &DhPubKey) -> Result<DhPubKey> {
    let mut shared_point = pubkey.clone();
    shared_point
        .0
        .tweak_mul_assign(&privkey.0)
        .map_err(|e| anyhow!("error: {:?}", e))?;

    Ok(shared_point)
}

fn gen_out_buf(pubkey: &PublicKey, shared_point: &DhPubKey) -> Result<[u8; 32]> {
    let mut master = Vec::with_capacity(COMPRESSED_PUBLIC_KEY_SIZE * 2);
    master.extend(pubkey.serialize_compressed().iter());
    master.extend(shared_point.0.serialize_compressed().iter());

    let mut out_buf = [0u8; 32];
    hkdf::expand(
        &HmacKey::from(master),
        b"dh",
        &mut out_buf,
        hkdf::Aes256GcmKey,
    )?;
    Ok(out_buf)
}
