#![cfg_attr(not(test), no_std)]

use serde_derive::{Deserialize, Serialize};

// we could use new-type pattern here but let's keep it simple
pub type Id = u32;
pub type DevId = u32;
pub type Parameter = u32;

use core::mem::size_of;
use corncobs::max_encoded_len;

pub const IN_SIZE: usize = max_encoded_len(size_of::<Ack>() + size_of::<u32>());
pub const OUT_SIZE: usize = max_encoded_len(size_of::<Command>() + size_of::<u32>());

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub enum Command {
    SetBlinker(BlinkerOptions),
    SetDateTime(DateTime),
    RgbOn,
    RgbOff,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BlinkerOptions {
    Off,
    On {
        date_time: DateTime,
        freq: u64,
        duration: u64,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum DateTime {
    Now,
    Utc(u64),
}

#[derive(Debug, Serialize, Deserialize)]
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
) -> &'a [u8] {
    let n_ser = ssmarshal::serialize(out_buf, t).unwrap();
    let crc = CKSUM.checksum(&out_buf[0..n_ser]);
    let n_crc = ssmarshal::serialize(&mut out_buf[n_ser..], &crc).unwrap();
    let buf_copy = *out_buf; // implies memcpy, could we do better?
    let n = corncobs::encode_buf(&buf_copy[0..n_ser + n_crc], out_buf);
    &out_buf[0..n]
}

/// deserialize T from cobs in_buf with crc check
/// panics on all errors
/// TODO: reasonable error handling
pub fn deserialize_crc_cobs<T>(in_buf: &mut [u8]) -> Result<T, ()>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let n = corncobs::decode_in_place(in_buf).unwrap();
    let (t, resp_used) = ssmarshal::deserialize::<T>(&in_buf[0..n]).unwrap();
    let crc_buf = &in_buf[resp_used..];
    let (crc, _crc_used) = ssmarshal::deserialize::<u32>(crc_buf).unwrap();
    let pkg_crc = CKSUM.checksum(&in_buf[0..resp_used]);
    assert_eq! {crc, pkg_crc};
    Ok(t)
}
