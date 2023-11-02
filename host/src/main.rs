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
use std::{io::Read, time::UNIX_EPOCH};

// Libraries
use corncobs::ZERO;
use dateparser::parse;
use serial2::SerialPort;
use std::io;
use std::io::Write;
use std::time::SystemTime;

// Application dependencies
use host::open;
use shared::{
    deserialize_crc_cobs, serialize_crc_cobs, Ack, BlinkerOptions, Command, IN_SIZE, OUT_SIZE,
};
// local library

type InBuf = [u8; IN_SIZE];
type OutBuf = [u8; OUT_SIZE];

fn main() -> Result<(), std::io::Error> {
    let mut port = open()?;
    let mut out_buf = [0u8; OUT_SIZE];
    let mut in_buf = [0u8; IN_SIZE];

    loop {
        println!(
            "\nTASKS:\n \
            1. Toggle RGB on\n \
            2. Toggle RGB off\n \
            3. Set blink data\n \
            4. Set date time\n \
            5. Quit\n"
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
            4 => Command::SetDateTime(shared::DateTime::Utc(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u64,
            )),
            5 => {
                break;
            }
            _ => {
                println!("Invalid task selected");
                continue;
            }
        };

        let response = request(&task, &mut port, &mut out_buf, &mut in_buf)?;
    }

    Ok(())
}

fn get_blink_data() -> BlinkerOptions {
    println!("\nInput \n <hh:mm:ss>\n <frequency>\n <duration>\n");

    let mut date_time = String::new();
    let mut frequency = String::new();
    let mut duration = String::new();

    println!("Insert date time <hh:mm:ss> or 'off' to set led off\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut date_time);

    if (date_time.trim().to_lowercase() == "off") {
        return BlinkerOptions::Off;
    }

    println!("\nInsert frequency (Hz)\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut frequency);

    println!("\nInsert duration in seconds\n");
    print!(" > ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut duration);

    let time = parse(&date_time.trim()).unwrap();
    return BlinkerOptions::On {
        date_time: shared::DateTime::Utc(time.timestamp() as u64),
        freq: frequency.trim().parse::<u64>().unwrap(),
        duration: duration.trim().parse::<u64>().unwrap(),
    };
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
