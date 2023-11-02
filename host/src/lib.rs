use serial2::SerialPort;
use std::io::Result;
use std::time::Duration;

// On Windows, use something like "COM1".
// For COM ports above COM9, you need to use the win32 device namespace, for example "\\.\COM10" (or "\\\\.\\COM10" with string escaping).
// For more details, see: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file?redirectedfrom=MSDN#win32-device-namespaces

#[cfg(target_os = "linux")]
static COM_PATH: &str = "/dev/ttyUSB0";
#[cfg(target_os = "windows")]
static COM_PATH: &str = "COM3";

// A one second timeout
const TIME_OUT: Duration = Duration::from_millis(1000);

pub fn open() -> Result<SerialPort> {
    let mut port = SerialPort::open(COM_PATH, 115200)?;
    // Needed for windows, but should not hurt on Linux
    port.set_dtr(true)?;
    port.set_rts(true)?;
    port.set_write_timeout(TIME_OUT)?;
    port.set_read_timeout(TIME_OUT)?;

    Ok(port)
}
