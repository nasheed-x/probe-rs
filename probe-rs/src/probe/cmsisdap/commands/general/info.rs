use super::super::{CommandId, Request, SendError};

macro_rules! info_command {
    ($id:expr, $name:ident, $response_type:ty) => {
        #[derive(Clone, Default, Debug)]
        pub struct $name {}

        impl Request for $name {
            const COMMAND_ID: CommandId = CommandId::Info;

            type Response = $response_type;

            fn to_bytes(&self, buffer: &mut [u8]) -> Result<usize, SendError> {
                buffer[0] = $id;
                Ok(1)
            }

            fn parse_response(&self, buffer: &[u8]) -> Result<Self::Response, SendError> {
                ParseFromResponse::from_response(buffer)
            }
        }
    };
}

info_command!(0x01, VendorCommand, Option<String>);

info_command!(0x02, ProductIdCommand, Option<String>);

info_command!(0x03, SerialNumberCommand, Option<String>);

info_command!(0x04, FirmwareVersionCommand, Option<String>);

info_command!(0x05, TargetDeviceVendorCommand, Option<String>);

info_command!(0x06, TargetDeviceNameCommand, Option<String>);

info_command!(0x07, TargetBoardVendorCommand, Option<String>);

info_command!(0x08, TargetBoardNameCommand, Option<String>);

info_command!(0xF0, CapabilitiesCommand, Capabilities);

#[derive(Copy, Clone, Debug)]
pub struct TestDomainTimeCommand {}

impl Request for TestDomainTimeCommand {
    const COMMAND_ID: CommandId = CommandId::Info;

    type Response = u32;

    fn to_bytes(&self, buffer: &mut [u8]) -> Result<usize, SendError> {
        buffer[0] = 0xF1;
        Ok(1)
    }
    fn parse_response(&self, buffer: &[u8]) -> Result<Self::Response, SendError> {
        if buffer.first() != Some(&0x08) {
            return Err(SendError::UnexpectedAnswer);
        }

        let available = buffer.get(1..).ok_or(SendError::NotEnoughData)?;
        if available.is_empty() {
            return Err(SendError::NotEnoughData);
        }
        if !cfg!(target_family = "wasm") && available.len() < 4 {
            return Err(SendError::NotEnoughData);
        }
        let mut value = [0; 4];
        let copied = available.len().min(value.len());
        value[..copied].copy_from_slice(&available[..copied]);
        Ok(u32::from_le_bytes(value))
    }
}

info_command!(0xFE, UartReceiveBufferSizeCommand, u32);
info_command!(0xFC, UartTransmitBufferSizeCommand, u32);
info_command!(0xFD, SWOTraceBufferSizeCommand, u32);
info_command!(0xFE, PacketCountCommand, u8);
info_command!(0xFF, PacketSizeCommand, u16);

trait ParseFromResponse: Sized {
    fn from_response(buffer: &[u8]) -> Result<Self, SendError>;
}

/// Parse a fixed-width DAP_Info payload while tolerating omitted high zeroes.
///
/// Some CMSIS-DAP v2 firmware sends a short bulk packet when the most
/// significant bytes of a little-endian value are zero. The response length
/// still advertises the field's full width, so the omitted tail is
/// unambiguous and can be safely restored as zeroes.
fn zero_padded_value<const N: usize>(buffer: &[u8]) -> Result<[u8; N], SendError> {
    if buffer.first() != Some(&(N as u8)) {
        return Err(SendError::UnexpectedAnswer);
    }

    let available = buffer.get(1..).ok_or(SendError::NotEnoughData)?;
    if available.is_empty() {
        return Err(SendError::NotEnoughData);
    }
    if !cfg!(target_family = "wasm") && available.len() < N {
        return Err(SendError::NotEnoughData);
    }

    let mut value = [0; N];
    let copied = available.len().min(N);
    value[..copied].copy_from_slice(&available[..copied]);
    Ok(value)
}

impl ParseFromResponse for Option<String> {
    /// Create a String out of the received buffer.
    ///
    /// The length of the buffer is read from the first byte of the buffer.
    /// If the length is zero, no string is returned.
    fn from_response(buffer: &[u8]) -> Result<Self, SendError> {
        let Some((&string_len, contents)) = buffer.split_first() else {
            return Err(SendError::NotEnoughData);
        };
        let string_len = string_len as usize; // including the zero terminator

        match string_len {
            0 => Ok(None),
            n => {
                let Some(contents) = contents.get(..n) else {
                    return Err(SendError::NotEnoughData);
                };
                let res = std::str::from_utf8(contents)?;
                Ok(Some(res.to_owned()))
            }
        }
    }
}

impl ParseFromResponse for u8 {
    fn from_response(buffer: &[u8]) -> Result<Self, SendError> {
        Ok(u8::from_le_bytes(zero_padded_value::<1>(buffer)?))
    }
}

impl ParseFromResponse for u16 {
    fn from_response(buffer: &[u8]) -> Result<Self, SendError> {
        Ok(u16::from_le_bytes(zero_padded_value::<2>(buffer)?))
    }
}

impl ParseFromResponse for u32 {
    fn from_response(buffer: &[u8]) -> Result<Self, SendError> {
        Ok(u32::from_le_bytes(zero_padded_value::<4>(buffer)?))
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Capabilities {
    pub(crate) response_length: u8,
    pub(crate) raw_primary: u8,
    pub(crate) swd_implemented: bool,
    pub(crate) jtag_implemented: bool,
    pub(crate) swo_uart_implemented: bool,
    pub(crate) swo_manchester_implemented: bool,
    pub(crate) _atomic_commands_implemented: bool,
    pub(crate) _test_domain_timer_implemented: bool,
    pub(crate) swo_streaming_trace_implemented: bool,
    pub(crate) _uart_communication_port_implemented: bool,
    pub(crate) _usb_com_port_implemented: bool,
}

impl ParseFromResponse for Capabilities {
    fn from_response(buffer: &[u8]) -> Result<Self, SendError> {
        // This response can contain two info bytes.
        // As described by https://arm-software.github.io/CMSIS-DAP/latest/group__DAP__Info.html
        let Some(&length) = buffer.first() else {
            return Err(SendError::NotEnoughData);
        };
        if length > 0 {
            let Some(&primary) = buffer.get(1) else {
                return Err(SendError::NotEnoughData);
            };
            let secondary = match buffer.get(2).copied() {
                Some(value) => value,
                None if cfg!(target_family = "wasm") => 0,
                None if length >= 2 => return Err(SendError::NotEnoughData),
                None => 0,
            };
            let capabilites = Capabilities {
                response_length: length,
                raw_primary: primary,
                swd_implemented: primary & 0x01 > 0,
                jtag_implemented: primary & 0x02 > 0,
                swo_uart_implemented: primary & 0x04 > 0,
                swo_manchester_implemented: primary & 0x08 > 0,
                _atomic_commands_implemented: primary & 0x10 > 0,
                _test_domain_timer_implemented: primary & 0x20 > 0,
                swo_streaming_trace_implemented: primary & 0x40 > 0,
                _uart_communication_port_implemented: primary & 0x80 > 0,
                _usb_com_port_implemented: if length >= 2 {
                    secondary & (1 << 0) != 0
                } else {
                    false
                },
            };

            Ok(capabilites)
        } else {
            Err(SendError::UnexpectedAnswer)
        }
    }
}
