extern crate libc;

use std::ffi::OsStr;
use std::io;
use std::mem;
use std::ptr;
use std::time::Duration;

use std::os::windows::prelude::*;

use self::libc::c_void;

use super::ffi::*;
use ::{SerialDevice,SerialPortSettings};


/// A serial port implementation for Windows COM ports.
///
/// The port will be closed when the value is dropped.
pub struct COMPort {
    handle: HANDLE,
    timeout: Duration
}

unsafe impl Send for COMPort {}

impl COMPort {
    /// Opens a COM port as a serial device.
    ///
    /// `port` should be the name of a COM port, e.g., `COM1`.
    ///
    /// ```no_run
    /// serial::windows::COMPort::open("COM1").unwrap();
    /// ```
    ///
    /// ## Errors
    ///
    /// * `NoDevice` if the device could not be opened. This could indicate that the device is
    ///   already in use.
    /// * `InvalidInput` if `port` is not a valid device name.
    /// * `Io` for any other I/O error while opening or initializing the device.
    pub fn open<T: AsRef<OsStr> + ?Sized>(port: &T) -> ::Result<Self> {
        let mut name = Vec::<u16>::new();

        name.extend(OsStr::new("\\\\.\\").encode_wide());
        name.extend(port.as_ref().encode_wide());
        name.push(0);

        let handle = unsafe {
            CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, ptr::null_mut(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0 as HANDLE)
        };

        let timeout = Duration::from_millis(100);

        if handle != INVALID_HANDLE_VALUE {
            let mut port = COMPort {
                handle: handle,
                timeout: timeout
            };

            try!(port.set_timeout(timeout));
            Ok(port)
        }
        else {
            Err(super::error::last_os_error())
        }
    }

    fn escape_comm_function(&mut self, function: DWORD) -> ::Result<()> {
        match unsafe { EscapeCommFunction(self.handle, function) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }

    fn read_pin(&mut self, pin: DWORD) -> ::Result<bool> {
        let mut status: DWORD = unsafe { mem::uninitialized() };

        match unsafe { GetCommModemStatus(self.handle, &mut status) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(status & pin != 0)
        }
    }

    /// Primative method for pulling information about device driver
    /// For full details see: https://msdn.microsoft.com/en-us/library/windows/desktop/aa363189%28v=vs.85%29.aspx
    pub fn driver_properties(&mut self) -> ::Result<Box<LPCOMMPROP>> {
        let b = Box::new(LPCOMMPROP::default());
        let ptr: *mut LPCOMMPROP = unsafe{ mem::transmute(b)};
        match unsafe{ GetCommProperties( self.handle, ptr) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(unsafe{mem::transmute(ptr)})
        }
    }

    /// Get # of bytes in RX_Buffer
    pub fn tx_buffer(&mut self) -> ::Result<u32> {
        match self.driver_properties()  {
            Err(e) => Err(e),
            Ok(x) => Ok(x.dwCurrentTxQueue)
        }
    }

    /// Get # of bytes in RX Buffer
    pub fn rx_buffer(&mut self) -> ::Result<u32> {
        match self.driver_properties() {
            Err(e) => Err(e),
            Ok(x) => Ok(x.dwCurrentRxQueue)
        }
    }

    /// Purge operations
    pub fn purge_rxabort(&mut self) -> ::Result<()> {
        match unsafe{ PurgeComm(self.handle, PURGE_RXABORT ) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }
    pub fn purge_rxclear(&mut self) -> ::Result<()> {
        match unsafe{ PurgeComm(self.handle, PURGE_RXCLEAR ) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }
    pub fn purge_txabort(&mut self) -> ::Result<()> {
        match unsafe{ PurgeComm(self.handle, PURGE_TXABORT ) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }
    pub fn purge_txclear(&mut self) -> ::Result<()> {
        match unsafe{ PurgeComm(self.handle, PURGE_TXCLEAR ) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }

    /// status operations
    pub fn bytes_in(&mut self) -> ::Result<u64> {
        let b = Box::new(COMSTAT::default());
        let p: *mut COMSTAT = unsafe{ mem::transmute(b)};
        let mut status: DWORD = unsafe { mem::uninitialized() };
        match unsafe{ ClearCommError( self.handle, &mut status, p ) } {
            0 => Err(super::error::last_os_error()),
            _ => {
                let p: &COMSTAT = unsafe{ mem::transmute(p)};
                Ok(p.cbInQue as u64)
            }
        }
    }
    /// status operations
    pub fn bytes_out(&mut self) -> ::Result<u64> {
        let b = Box::new(COMSTAT::default());
        let p: *mut COMSTAT = unsafe{ mem::transmute(b)};
        let mut status: DWORD = unsafe { mem::uninitialized() };
        match unsafe{ ClearCommError( self.handle, &mut status, p ) } {
            0 => Err(super::error::last_os_error()),
            _ => {
                let p: &COMSTAT = unsafe{ mem::transmute(p)};
                Ok(p.cbOutQue as u64)
            }
        }
    }

    /// write settings
    pub fn write_settings(&mut self, settings: &COMSettings) -> ::Result<()> {
        match unsafe { SetCommState(self.handle, &settings.inner) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }
}

impl Drop for COMPort {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

impl AsRawHandle for COMPort {
    fn as_raw_handle(&self) -> RawHandle {
        unsafe {
            mem::transmute(self.handle)
        }
    }
}

impl io::Read for COMPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut len: DWORD = 0;

        match unsafe { ReadFile(self.handle, buf.as_mut_ptr() as *mut c_void, buf.len() as DWORD, &mut len, ptr::null_mut()) } {
            0 => Err(io::Error::last_os_error()),
            _ => {
                if len != 0 {
                    Ok(len as usize)
                }
                else {
                    Err(io::Error::new(io::ErrorKind::TimedOut, "Operation timed out"))
                }
            }
        }
    }
}

impl io::Write for COMPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut len: DWORD = 0;

        match unsafe { WriteFile(self.handle, buf.as_ptr() as *mut c_void, buf.len() as DWORD, &mut len, ptr::null_mut()) } {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(len as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match unsafe { FlushFileBuffers(self.handle) } {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(())
        }
    }
}

impl SerialDevice for COMPort {
    type Settings = COMSettings;

    fn read_settings(&self) -> ::Result<COMSettings> {
        let mut dcb = DCB::new();

        match unsafe { GetCommState(self.handle, &mut dcb) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(COMSettings { inner: dcb })

        }
    }

    fn write_settings(&mut self, settings: &COMSettings) -> ::Result<()> {
        match unsafe { SetCommState(self.handle, &settings.inner) } {
            0 => Err(super::error::last_os_error()),
            _ => Ok(())
        }
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn set_timeout(&mut self, timeout: Duration) -> ::Result<()> {
        let milliseconds = timeout.as_secs() * 1000 + timeout.subsec_nanos() as u64 / 1_000_000;

        let timeouts = COMMTIMEOUTS {
            ReadIntervalTimeout: 0,
            ReadTotalTimeoutMultiplier: 0,
            ReadTotalTimeoutConstant: milliseconds as DWORD,
            WriteTotalTimeoutMultiplier: 0,
            WriteTotalTimeoutConstant: 0
        };

        if unsafe { SetCommTimeouts(self.handle, &timeouts) } == 0 {
            return Err(super::error::last_os_error());
        }

        self.timeout = timeout;
        Ok(())
    }

    fn set_rts(&mut self, level: bool) -> ::Result<()> {
        if level {
            self.escape_comm_function(SETRTS)
        }
        else {
            self.escape_comm_function(CLRRTS)
        }
    }

    fn set_dtr(&mut self, level: bool) -> ::Result<()> {
        if level {
            self.escape_comm_function(SETDTR)
        }
        else {
            self.escape_comm_function(CLRDTR)
        }
    }

    fn read_cts(&mut self) -> ::Result<bool> {
        self.read_pin(MS_CTS_ON)
    }

    fn read_dsr(&mut self) -> ::Result<bool> {
        self.read_pin(MS_DSR_ON)
    }

    fn read_ri(&mut self) -> ::Result<bool> {
        self.read_pin(MS_RING_ON)
    }

    fn read_cd(&mut self) -> ::Result<bool> {
        self.read_pin(MS_RLSD_ON)
    }
}


/// Serial port settings for COM ports.
#[derive(Copy,Clone,Debug)]
pub struct COMSettings {
    inner: DCB
}

impl SerialPortSettings for COMSettings {
    fn baud_rate(&self) -> Option<::BaudRate> {
        match self.inner.BaudRate {
            CBR_110    => Some(::Baud110),
            CBR_300    => Some(::Baud300),
            CBR_600    => Some(::Baud600),
            CBR_1200   => Some(::Baud1200),
            CBR_2400   => Some(::Baud2400),
            CBR_4800   => Some(::Baud4800),
            CBR_9600   => Some(::Baud9600),
            CBR_14400  => Some(::BaudOther(14400)),
            CBR_19200  => Some(::Baud19200),
            CBR_38400  => Some(::Baud38400),
            CBR_56000  => Some(::BaudOther(56000)),
            CBR_57600  => Some(::Baud57600),
            CBR_115200 => Some(::Baud115200),
            CBR_128000 => Some(::BaudOther(128000)),
            CBR_256000 => Some(::BaudOther(256000)),
            n          => Some(::BaudOther(n as usize))
        }
    }

    fn char_size(&self) -> Option<::CharSize> {
        match self.inner.ByteSize {
            5 => Some(::Bits5),
            6 => Some(::Bits6),
            7 => Some(::Bits7),
            8 => Some(::Bits8),
            _ => None
        }
    }

    fn parity(&self) -> Option<::Parity> {
        match self.inner.Parity {
            ODDPARITY  => Some(::ParityOdd),
            EVENPARITY => Some(::ParityEven),
            NOPARITY   => Some(::ParityNone),
            _          => None
        }
    }

    fn stop_bits(&self) -> Option<::StopBits> {
        match self.inner.StopBits {
            TWOSTOPBITS => Some(::Stop2),
            ONESTOPBIT  => Some(::Stop1),
            _           => None
        }
    }

    fn flow_control(&self) -> Option<::FlowControl> {
        if self.inner.fBits & (fOutxCtsFlow | fRtsControl) != 0 {
            Some(::FlowHardware)
        }
        else if self.inner.fBits & (fOutX | fInX) != 0 {
            Some(::FlowSoftware)
        }
        else {
            Some(::FlowNone)
        }
    }

    fn set_baud_rate(&mut self, baud_rate: ::BaudRate) -> ::Result<()> {
        self.inner.BaudRate = match baud_rate {
            ::Baud110      => CBR_110,
            ::Baud300      => CBR_300,
            ::Baud600      => CBR_600,
            ::Baud1200     => CBR_1200,
            ::Baud2400     => CBR_2400,
            ::Baud4800     => CBR_4800,
            ::Baud9600     => CBR_9600,
            ::Baud19200    => CBR_19200,
            ::Baud38400    => CBR_38400,
            ::Baud57600    => CBR_57600,
            ::Baud115200   => CBR_115200,
            ::BaudOther(n) => n as DWORD
        };

        Ok(())
    }

    fn set_char_size(&mut self, char_size: ::CharSize) {
        self.inner.ByteSize = match char_size {
            ::Bits5 => 5,
            ::Bits6 => 6,
            ::Bits7 => 7,
            ::Bits8 => 8
        }
    }

    fn set_parity(&mut self, parity: ::Parity) {
        self.inner.Parity = match parity {
            ::ParityNone => NOPARITY,
            ::ParityOdd  => ODDPARITY,
            ::ParityEven => EVENPARITY
        }
    }

    fn set_stop_bits(&mut self, stop_bits: ::StopBits) {
        self.inner.StopBits = match stop_bits {
            ::Stop1 => ONESTOPBIT,
            ::Stop2 => TWOSTOPBITS
        }
    }

    fn set_flow_control(&mut self, flow_control: ::FlowControl) {
        match flow_control {
            ::FlowNone => {
                self.inner.fBits &= !(fOutxCtsFlow | fRtsControl);
                self.inner.fBits &= !(fOutX | fInX);
            },
            ::FlowSoftware => {
                self.inner.fBits &= !(fOutxCtsFlow | fRtsControl);
                self.inner.fBits |= fOutX | fInX;
            },
            ::FlowHardware => {
                self.inner.fBits |= fOutxCtsFlow | fRtsControl;
                self.inner.fBits &= !(fOutX | fInX);
            }
        }
    }
}
