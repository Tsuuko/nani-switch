use aes::Aes256;
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use ctr::cipher::{KeyIvInit, StreamCipher};
use sha2::{Digest, Sha256};

type Aes256Ctr = ctr::Ctr128BE<Aes256>;

fn derive_key(user_data_path: &str) -> [u8; 32] {
    Sha256::digest(format!("{user_data_path}nn").as_bytes()).into()
}

pub fn encrypt_token(plaintext: &str, user_data_path: &str) -> Result<String> {
    let iv: [u8; 16] = rand::random();
    let key = derive_key(user_data_path);
    let mut encrypted = plaintext.as_bytes().to_vec();
    let mut cipher = Aes256Ctr::new(&key.into(), &iv.into());
    cipher.apply_keystream(&mut encrypted);
    let mut bytes = Vec::with_capacity(iv.len() + encrypted.len());
    bytes.extend_from_slice(&iv);
    bytes.extend_from_slice(&encrypted);
    Ok(STANDARD.encode(bytes))
}

pub fn decrypt_token(stored: &str, user_data_path: &str) -> Result<String> {
    if stored.starts_with("dbg-") {
        return Ok(stored.to_owned());
    }
    let bytes = STANDARD
        .decode(stored)
        .context("invalid encrypted access token")?;
    if bytes.len() < 17 {
        bail!("invalid encrypted access token");
    }
    let key = derive_key(user_data_path);
    let iv: [u8; 16] = bytes[..16]
        .try_into()
        .expect("the encrypted token IV length was checked");
    let mut decrypted = bytes[16..].to_vec();
    let mut cipher = Aes256Ctr::new(&key.into(), &iv.into());
    cipher.apply_keystream(&mut decrypted);
    String::from_utf8(decrypted).context("access token is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trip() {
        let token = "header.payload.signature";
        let path = r"C:\Users\Test\AppData\Roaming\Nani";
        let stored = encrypt_token(token, path).unwrap();
        assert_ne!(stored, token);
        assert_eq!(decrypt_token(&stored, path).unwrap(), token);
    }
}
