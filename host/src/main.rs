//! host side application
//!
//! Run on target `cd esp32c3`
//!
//! cargo embed --example cmd_crc_cobs_lib --release
//!
//! Run on host `cd host`
//!
//! cargo run
//!

// Rust dependencies
use std::{io::Read, mem::size_of};

// Libraries
use corncobs::{max_encoded_len, ZERO};
use serial2::SerialPort;

// Application dependencies
use host::open;
use shared::{deserialize_crc_cobs, serialize_crc_cobs, Ack, Command}; // local library

const IN_SIZE: usize = max_encoded_len(size_of::<Ack>() + size_of::<u32>());
const OUT_SIZE: usize = max_encoded_len(size_of::<Command>() + size_of::<u32>());

type InBuf = [u8; IN_SIZE];
type OutBuf = [u8; OUT_SIZE];

fn main() -> Result<(), std::io::Error> {
    let mut port = open()?;

    let mut out_buf = [0u8; OUT_SIZE];
    let mut in_buf = [0u8; IN_SIZE];

    let cmd_off = Command::SetBlinker(shared::BlinkerOptions::Off);
    let cmd_on = Command::SetBlinker(shared::BlinkerOptions::On {
        date_time: shared::DateTime::Now,
        freq: 2,
        duration: 100,
    });

    println!("request {:?}", cmd_on);
    let response = request(&cmd_on, &mut port, &mut out_buf, &mut in_buf)?;
    println!("response {:?}", response);

    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("request {:?}", cmd_off);
    let response = request(&cmd_off, &mut port, &mut out_buf, &mut in_buf)?;
    println!("response {:?}", response);
    Ok(())
}

fn request(
    cmd: &Command,
    port: &mut SerialPort,
    out_buf: &mut OutBuf,
    in_buf: &mut InBuf,
) -> Result<Ack, std::io::Error> {
    println!("out_buf {}", out_buf.len());
    let to_write = serialize_crc_cobs(cmd, out_buf);
    port.write_all(to_write)?;

    let mut index: usize = 0;
    loop {
        let slice = &mut in_buf[index..index + 1];
        if index < IN_SIZE {
            index += 1;
        }
        port.read_exact(slice)?;
        if slice[0] == ZERO {
            println!("-- cobs package received --");
            break;
        }
    }
    println!("cobs index {}", index);
    Ok(deserialize_crc_cobs(in_buf).unwrap())
}
