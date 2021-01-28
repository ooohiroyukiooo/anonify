#![allow(dead_code)]

use crate::bincode;
use crate::crypto_box::{self, aead::Aead, Box as CryptoBox, PublicKey, SecretKey, KEY_SIZE};
use crate::local_anyhow::{anyhow, Result};
use crate::localstd::{boxed::Box, fmt, vec::Vec};
#[cfg(feature = "sgx")]
use crate::rng::SgxRng;
use crate::serde::{
    de::{self, SeqAccess, Unexpected},
    ser::{self, SerializeTuple},
    Deserialize, Serialize, Serializer,
};
use crate::xsalsa20poly1305::{Nonce, NONCE_SIZE};

// PublicKey in crypto_box is defined in x25519-dalek, which have 32 bytes length.
// see: https://github.com/dalek-cryptography/x25519-dalek/blob/0985e1babf0ba03d151b864ee28baee564662a8d/src/x25519.rs#L35
const PUBLIC_KEY_SIZE: usize = 32;

#[derive(Debug, Clone, Default)]
pub struct SodiumNonce(Nonce);

impl Serialize for SodiumNonce {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let contents = self.as_slice();
        if contents.len() != NONCE_SIZE {
            return Err(ser::Error::custom(
                "a SodiumNonce must have 24 bytes length",
            ));
        }

        let mut tup = serializer.serialize_tuple(NONCE_SIZE)?;
        for byte in contents.iter() {
            tup.serialize_element(byte)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for SodiumNonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct SodiumNonceVisitor;

        impl<'de> de::Visitor<'de> for SodiumNonceVisitor {
            type Value = SodiumNonce;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SodiumNonce must be 24 bytes length")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(SodiumNonce::from_bytes(value))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut bytes = [0u8; KEY_SIZE];
                for i in 0..KEY_SIZE {
                    bytes[i] = seq
                        .next_element()?
                        .ok_or(de::Error::invalid_length(i, &"expected 24 bytes"))?;
                }
                Ok(SodiumNonce::from_bytes(&bytes))
            }
        }

        deserializer.deserialize_tuple(NONCE_SIZE, SodiumNonceVisitor)
    }
}

impl SodiumNonce {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        SodiumNonce(*Nonce::from_slice(bytes))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn from_random() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            let mut rng = rand::thread_rng();
            let inner = crypto_box::generate_nonce(&mut rng);
            Ok(SodiumNonce(inner))
        }

        #[cfg(feature = "sgx")]
        {
            let mut rng = SgxRng::new()?;
            let inner = crypto_box::generate_nonce(&mut rng);
            Ok(SodiumNonce(inner))
        }
    }
}

#[derive(Debug, Clone)]
pub struct SodiumPrivateKey(SecretKey);

impl Default for SodiumPrivateKey {
    fn default() -> Self {
        let inner = SecretKey::from([0u8; KEY_SIZE]);
        SodiumPrivateKey(inner)
    }
}

impl Serialize for SodiumPrivateKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(KEY_SIZE)?;
        for byte in self.0.to_bytes().iter() {
            tup.serialize_element(byte)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for SodiumPrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct SodiumPrivateKeyVisitor;

        impl<'de> de::Visitor<'de> for SodiumPrivateKeyVisitor {
            type Value = SodiumPrivateKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SodiumPrivateKey must be 32 bytes length")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let sk = SodiumPrivateKey::from_bytes(value).map_err(|e| E::custom(e))?;

                Ok(sk)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut bytes = [0u8; KEY_SIZE];
                for i in 0..KEY_SIZE {
                    bytes[i] = seq
                        .next_element()?
                        .ok_or(de::Error::invalid_length(i, &"expected 32 bytes"))?;
                }
                let sk = SodiumPrivateKey::from_bytes(&bytes).map_err(|_e| {
                    de::Error::invalid_value(
                        Unexpected::Bytes(&bytes[..]),
                        &"Fail SodiumPrivateKey::from_bytes",
                    )
                })?;

                Ok(sk)
            }
        }

        deserializer.deserialize_tuple(KEY_SIZE, SodiumPrivateKeyVisitor)
    }
}

impl SodiumPrivateKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != KEY_SIZE {
            return Err(anyhow!(
                "SodiumPrivateKey's length must be {}, got {}",
                KEY_SIZE,
                bytes.len()
            ));
        }
        let mut buf = [0u8; KEY_SIZE];
        buf.copy_from_slice(&bytes[..KEY_SIZE]);
        let inner = SecretKey::from(buf);
        Ok(SodiumPrivateKey(inner))
    }

    pub fn from_random() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            let mut rng = rand::thread_rng();
            let inner = SecretKey::generate(&mut rng);
            Ok(SodiumPrivateKey(inner))
        }

        #[cfg(feature = "sgx")]
        {
            let mut rng = SgxRng::new()?;
            let inner = SecretKey::generate(&mut rng);
            Ok(SodiumPrivateKey(inner))
        }
    }

    pub fn public_key(&self) -> SodiumPubKey {
        SodiumPubKey(self.0.public_key())
    }
}

#[derive(Debug, Clone)]
pub struct SodiumPubKey(PublicKey);

impl Default for SodiumPubKey {
    fn default() -> Self {
        let inner = SodiumPrivateKey::default().0.public_key();
        SodiumPubKey(inner)
    }
}

impl Serialize for SodiumPubKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(PUBLIC_KEY_SIZE)?;
        for byte in self.to_bytes().iter() {
            tup.serialize_element(byte)?;
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for SodiumPubKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct SodiumPubKeyVisitor;

        impl<'de> de::Visitor<'de> for SodiumPubKeyVisitor {
            type Value = SodiumPubKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SodiumPubKey must be 32 bytes length")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut bytes = [0u8; PUBLIC_KEY_SIZE];
                for i in 0..PUBLIC_KEY_SIZE {
                    bytes[i] = seq
                        .next_element()?
                        .ok_or(de::Error::invalid_length(i, &"expected 32 bytes"))?;
                }
                let pk = SodiumPubKey::from_bytes(&bytes).map_err(|_e| {
                    de::Error::invalid_value(
                        Unexpected::Bytes(&bytes[..]),
                        &"Fail SodiumPubKey::from_bytes",
                    )
                })?;

                Ok(pk)
            }
        }

        deserializer.deserialize_tuple(PUBLIC_KEY_SIZE, SodiumPubKeyVisitor)
    }
}

impl SodiumPubKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PUBLIC_KEY_SIZE {
            return Err(anyhow!(
                "SodiumPubKey's length must be {}, got {}",
                PUBLIC_KEY_SIZE,
                bytes.len()
            ));
        }
        let mut buf = [0u8; PUBLIC_KEY_SIZE];
        buf.copy_from_slice(&bytes[..PUBLIC_KEY_SIZE]);
        let inner = PublicKey::from(buf);
        Ok(SodiumPubKey(inner))
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        self.0.to_bytes()
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap() // must not fail
    }

    pub fn decode(bytes: &[u8]) -> crate::localstd::result::Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize(bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(crate = "crate::serde")]
pub struct SodiumCiphertext {
    ephemeral_public_key: SodiumPubKey,
    nonce: SodiumNonce,
    #[serde(with = "crate::serde_bytes")]
    ciphertext: Vec<u8>,
}

impl SodiumCiphertext {
    pub fn encrypt(others_pub_key: &SodiumPubKey, plaintext: Vec<u8>) -> Result<Self> {
        let my_ephemeral_secret = SodiumPrivateKey::from_random()?;
        let my_ephemeral_pub_key = my_ephemeral_secret.public_key();
        let nonce = SodiumNonce::from_random()?;

        let cbox = CryptoBox::new(&others_pub_key.0, &my_ephemeral_secret.0);
        let ciphertext = cbox
            .encrypt(&nonce.0, &plaintext[..])
            .map_err(|e| anyhow!("Failed to encrypt :{:?}", e))?;

        Ok(SodiumCiphertext {
            ephemeral_public_key: my_ephemeral_pub_key,
            ciphertext,
            nonce,
        })
    }

    pub fn decrypt(self, my_priv_key: &SodiumPrivateKey) -> Result<Vec<u8>> {
        let cbox = CryptoBox::new(&self.ephemeral_public_key.0, &my_priv_key.0);
        let plaintext = cbox.decrypt(&self.nonce.0, &self.ciphertext[..]).unwrap();

        Ok(plaintext)
    }

    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap() // must not fail
    }
}

#[cfg(feature = "sgx")]
#[cfg(debug_assertions)]
pub(crate) mod tests {
    use super::*;
    use crate::localstd::string::String;
    use test_utils::*;

    pub(crate) fn run_tests() -> bool {
        run_tests!(test_sodium,)
    }

    fn test_sodium() {
        let sk_server = SodiumPrivateKey::from_random().unwrap();
        let pk_server = sk_server.public_key();

        let msg = b"This is a test message";
        let ciphertext = SodiumCiphertext::encrypt(&pk_server, msg.to_vec()).unwrap();

        let plaintext = ciphertext.decrypt(&sk_server).unwrap();
        assert_eq!(plaintext, &msg[..]);
    }
}
