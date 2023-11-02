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
use std::io;
use std::io::Write;

// Application dependencies
use host::open;
use shared::{deserialize_crc_cobs, serialize_crc_cobs, Ack, Command, IN_SIZE, OUT_SIZE}; // local library

type InBuf = [u8; IN_SIZE];
type OutBuf = [u8; OUT_SIZE];

fn main() -> Result<(), std::io::Error> {
    let mut port = open()?;
    let mut out_buf = [0u8; OUT_SIZE];
    let mut in_buf = [0u8; IN_SIZE];

    loop {
        println!("\nTASKS: \n 1. Toggle RGB on \n 2. Toggle RGB off \n 3. Set blink data\n 4. Set date time\n");
        print!(" > ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        let task: u32 = match input.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("Invalid input");
                continue;
            }
        };

        match task {
            1 => {
                let response = request(&Command::RgbOn, &mut port, &mut out_buf, &mut in_buf)?;
            }
            2 => {
                let response = request(&Command::RgbOff, &mut port, &mut out_buf, &mut in_buf)?;
            }
            3 => {
                println!("3");
            }
            4 => {
                println!("4");
            }
            _ => {
                println!("Invalid task selected");
                continue;
            }
        }
    }

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
