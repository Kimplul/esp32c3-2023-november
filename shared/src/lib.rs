#![cfg_attr(not(test), no_std)]
#![feature(iter_array_chunks)]
use hamming::{encode_hamming};
use serde_derive::{Deserialize, Serialize};
pub mod hamming;

// we could use new-type pattern here but let's keep it simple
pub type Id = u32;
pub type DevId = u32;
pub type Parameter = u32;

use core::mem::size_of;
use corncobs::max_encoded_len;

pub const IN_SIZE: usize = max_encoded_len(size_of::<Ack>() + size_of::<u32>()) * 2;
pub const OUT_SIZE: usize = max_encoded_len(size_of::<Command>() + size_of::<u32>()) * 2;

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum Command {
    SetBlinker(BlinkerOptions),
    SetDateTime(DateTime),
    RgbOn,
    RgbOff,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[repr(C)]
pub enum BlinkerOptions {
    Off,
    On {
        date_time: DateTime,
        freq: u64,
        duration: u64,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[repr(C)]
pub enum DateTime {
    Now,
    Utc(u64),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[repr(C)]
pub enum Ack {
    Ok,
    Recovered,
    NotOk,
}

pub const CKSUM: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_CKSUM);

/// Serialize T into cobs encoded out_buf with crc
/// panics on all errors
/// TODO: reasonable error handling
pub fn serialize_crc_cobs<'a, T: serde::Serialize, const N: usize>(
    t: &T,
    out_buf: &'a mut [u8; N],
) -> &'a mut [u8] {
    let n_ser = ssmarshal::serialize(out_buf, t).unwrap();
    let crc = CKSUM.checksum(&out_buf[0..n_ser]);
    let n_crc = ssmarshal::serialize(&mut out_buf[n_ser..], &crc).unwrap();
    let buf_copy = *out_buf; // implies memcpy, could we do better?
    let n = corncobs::encode_buf(&buf_copy[0..n_ser + n_crc], out_buf);
    let temp = *out_buf;

    let mut idx = 0;

    for b in &temp[0..n] {
        let first_half = b & 0xF;
        let second_half = b >> 4 & 0xF;
        let firsthalf_encoder = encode_hamming(first_half);
        let secondhalf_encoder = encode_hamming(second_half);
        out_buf[idx] = firsthalf_encoder;

        idx += 1;
        out_buf[idx] = secondhalf_encoder;
        idx += 1;
    }

    &mut out_buf[0..idx]
}

#[derive(Debug)]
pub enum DeserializeError {
    DecodeError,
    DeserializeError,
    CrcError,
    HammingError,
}
/// deserialize T from cobs in_buf with crc check
/// panics on all errors
/// TODO: reasonable error handling
pub fn deserialize_crc_cobs<T>(in_buf: &mut [u8]) -> Result<T, DeserializeError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    /* looks kind of interesting */
    let n = corncobs::decode_in_place(in_buf);
    let n = match n {
        Ok(n) => n,
        Err(_) => return Err(DeserializeError::DecodeError),
    };

    let r = ssmarshal::deserialize::<T>(&in_buf[0..n]);
    let (t, resp_used) = match r {
        Ok((t, resp_used)) => (t, resp_used),
        Err(_) => return Err(DeserializeError::DeserializeError),
    };

    let crc_buf = &in_buf[resp_used..];
    let r = ssmarshal::deserialize::<u32>(crc_buf);
    let (crc, _crc_used) = match r {
        Ok((crc, _crc_used)) => (crc, _crc_used),
        Err(_) => return Err(DeserializeError::DeserializeError),
    };

    let pkg_crc = CKSUM.checksum(&in_buf[0..resp_used]);

    if crc != pkg_crc {
        return Err(DeserializeError::CrcError);
    }
    Ok(t)
}
