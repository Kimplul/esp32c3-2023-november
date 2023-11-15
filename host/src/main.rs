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
use std::io::Read;

// Libraries
use corncobs::ZERO;
use dateparser::{parse, parse_with_timezone};
use serial2::SerialPort;
use std::io;
use std::io::Write;

// Application dependencies
use host::open;
use shared::{
    deserialize_crc_cobs, serialize_crc_cobs, Ack, BlinkerOptions, Command, DateTime, IN_SIZE,
    OUT_SIZE,
};
// local library

type InBuf = [u8; IN_SIZE];
type OutBuf = [u8; OUT_SIZE];

fn main() -> Result<(), std::io::Error> {
    let mut port = open()?;

    loop {
        let mut bitflip_payload = false;
        let mut out_buf = [0u8; OUT_SIZE];
        let mut in_buf = [0u8; IN_SIZE];

        println!(
            "\nTASKS:\n \
            1. Toggle RGB on\n \
            2. Toggle RGB off\n \
            3. Set blink data\n \
            4. Set date time\n \
            5. Bit flip on payload\n \
            6. Quit\n"
        );
        print!(" > ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        let command: u32 = match input.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("Invalid input");
                continue;
            }
        };

        let task = match command {
            1 => Command::RgbOn,
            2 => Command::RgbOff,
            3 => Command::SetBlinker(get_blink_data()),
            4 => Command::SetDateTime(set_datetime()),
            5 => {
                bitflip_payload = true;
                Command::RgbOn
            }
            6 => {
                break;
            }
            _ => {
                println!("Invalid task selected");
                continue;
            }
        };

        let response = request(&task, &mut port, &mut out_buf, &mut in_buf, bitflip_payload)?;
    }

    Ok(())
}

fn get_blink_data() -> BlinkerOptions {
    println!("\nInput \n <hh:mm:ss>, <off>, <now>\n <frequency>\n <duration>\n");

    let mut date_time_string = String::new();
    let mut frequency = String::new();
    let mut duration = String::new();

    println!("Insert date time <hh:mm:ss> or 'off' to set led off\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut date_time_string);

    if date_time_string.trim().to_lowercase() == "off" {
        return BlinkerOptions::Off;
    }

    let date_time;

    if date_time_string.trim().to_lowercase() == "now" {
        date_time = shared::DateTime::Now;
    } else {
        let date_time_ = parse(&date_time_string.trim()).unwrap();
        date_time = shared::DateTime::Utc(date_time_.timestamp() as u64);
    }

    println!("\nInsert frequency (Hz)\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut frequency);

    let freq = frequency.trim().parse::<u64>().unwrap();

    println!("\nInsert duration in seconds\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut duration);

    let duration = duration.trim().parse::<u64>().unwrap();

    BlinkerOptions::On {
        date_time,
        freq,
        duration,
    }
}

fn set_datetime() -> DateTime {
    let mut date_time_string = String::new();

    println!("Insert date time <hh:mm:ss> or 'now' to set current time\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read_line(&mut date_time_string);

    // Use naive_local time to ignore timezone and pretend that our local timzone is UTC0.
    if date_time_string.trim().to_lowercase() == "now" {
        let utc_timestamp = chrono::Local::now().naive_local().timestamp();
        return shared::DateTime::Utc(utc_timestamp as u64);
    }
    // Using UTC timezone to pretend that our local timezone is UTC0.
    let date_time_ = parse_with_timezone(date_time_string.trim(), &chrono::Utc).unwrap();
    shared::DateTime::Utc(date_time_.naive_local().timestamp() as u64)
}

fn request(
    cmd: &Command,
    port: &mut SerialPort,
    out_buf: &mut OutBuf,
    in_buf: &mut InBuf,
    bitflip_payload: bool,
) -> Result<Ack, std::io::Error> {
    println!("out_buf {}", out_buf.len());
    let to_write = serialize_crc_cobs(cmd, out_buf);

    if bitflip_payload {
        to_write[1] ^= 1 << 0;
    }
    println!("{:?}", to_write);

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
