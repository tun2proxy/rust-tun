//            DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
//                    Version 2, December 2004
//
// Copyleft (ↄ) meh. <meh@schizofreni.co> | http://meh.schizofreni.co
//
// Everyone is permitted to copy and distribute verbatim or modified
// copies of this license document, and changing it is allowed as long
// as the name is changed.
//
//            DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
//   TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION
//
//  0. You just DO WHAT THE FUCK YOU WANT TO.

use crate::platform::posix::Fd;
use crate::PACKET_INFORMATION_LENGTH as PIL;
use bytes::BufMut;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::sync::RwLock;

/// Infer the protocol based on the first nibble in the packet buffer.
pub(crate) fn is_ipv6(buf: &[u8]) -> std::io::Result<bool> {
    use std::io::{Error, ErrorKind::InvalidData};
    if buf.is_empty() {
        return Err(Error::new(InvalidData, "Zero-length data"));
    }
    match buf[0] >> 4 {
        4 => Ok(false),
        6 => Ok(true),
        p => Err(Error::new(InvalidData, format!("IP version {}", p))),
    }
}

pub(crate) fn generate_packet_information(
    _packet_information: bool,
    _ipv6: bool,
) -> Option<[u8; PIL]> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TUN_PROTO_IP6: [u8; PIL] = (libc::ETH_P_IPV6 as u32).to_be_bytes();
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TUN_PROTO_IP4: [u8; PIL] = (libc::ETH_P_IP as u32).to_be_bytes();

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const TUN_PROTO_IP6: [u8; PIL] = (libc::AF_INET6 as u32).to_be_bytes();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const TUN_PROTO_IP4: [u8; PIL] = (libc::AF_INET as u32).to_be_bytes();

    // FIXME: Currently, the FreeBSD we test (FreeBSD-14.0-RELEASE) seems to have no PI. Here just a dummy.
    #[cfg(target_os = "freebsd")]
    const TUN_PROTO_IP6: [u8; PIL] = 0x86DD_u32.to_be_bytes();
    #[cfg(target_os = "freebsd")]
    const TUN_PROTO_IP4: [u8; PIL] = 0x0800_u32.to_be_bytes();

    #[cfg(unix)]
    if _packet_information {
        if _ipv6 {
            return Some(TUN_PROTO_IP6);
        } else {
            return Some(TUN_PROTO_IP4);
        }
    }
    None
}

#[rustversion::since(1.79)]
macro_rules! local_buf_util {
	($e:expr,$size:expr) => {
		if $e{
			&mut vec![0u8; $size][..]
		}else{
			const STACK_BUF_LEN: usize = crate::DEFAULT_MTU as usize + PIL;
			&mut [0u8; STACK_BUF_LEN]
		}
	};
}

#[rustversion::before(1.79)]
macro_rules! local_buf_util {
	($e:expr,$size:expr) =>{
		{
			pub(crate) enum OptBuf{
				Heap(Vec<u8>),
				Stack([u8;crate::DEFAULT_MTU as usize + PIL])
			}
			impl OptBuf{
				pub(crate) fn as_mut(& mut self)->& mut [u8]{
					match self{
						OptBuf::Heap(v)=>v.as_mut(),
						OptBuf::Stack(v)=>v.as_mut()
					}
				}
			}

			fn get_local_buf(cond:bool,in_buf_len:usize)-> OptBuf{
				if cond{
					OptBuf::Heap(vec![0u8; in_buf_len])
				}else{
					const STACK_BUF_LEN: usize = crate::DEFAULT_MTU as usize + PIL;
					OptBuf::Stack([0u8; STACK_BUF_LEN])
				}
			}
			get_local_buf($e,$size)
		}
	}
}

#[rustversion::since(1.79)]
macro_rules! need_mut {
    ($id:ident, $e:expr) => {
        let $id = $e;
    };
}

#[rustversion::before(1.79)]
macro_rules! need_mut {
    ($id:ident, $e:expr) => {
        let mut $id = $e;
    };
}

pub struct Tun {
    pub(crate) fd: Fd,
    pub(crate) offset: usize,
    pub(crate) mtu: RwLock<u16>,
    pub(crate) packet_information: bool,
}

impl Tun {
    pub(crate) fn new(fd: Fd, mtu: u16, packet_information: bool) -> Self {
        let offset = if packet_information { PIL } else { 0 };
        Self {
            fd,
            offset,
            mtu: RwLock::new(mtu),
            packet_information,
        }
    }

    pub fn set_nonblock(&self) -> io::Result<()> {
        self.fd.set_nonblock()
    }

    pub fn set_mtu(&self, value: u16) {
        *self.mtu.write().unwrap() = value;
    }

    pub fn mtu(&self) -> u16 {
        *self.mtu.read().unwrap()
    }

    pub fn packet_information(&self) -> bool {
        self.packet_information
    }

    pub(crate) fn send(&self, in_buf: &[u8]) -> io::Result<usize> {
        const STACK_BUF_LEN: usize = crate::DEFAULT_MTU as usize + PIL;
        let in_buf_len = in_buf.len() + self.offset;

        // The following logic is to prevent dynamically allocating Vec on every send
        // As long as the MTU is set to value lesser than 1500, this api uses `stack_buf`
        // and avoids `Vec` allocation
        let local_buf_v0 =
            local_buf_util!(in_buf_len > STACK_BUF_LEN && self.offset != 0, in_buf_len);
        need_mut! {local_buf_v1,local_buf_v0};
        #[allow(clippy::useless_asref)]
        let local_buf = local_buf_v1.as_mut();

        let either_buf = if self.offset != 0 {
            let ipv6 = is_ipv6(in_buf)?;
            if let Some(header) = generate_packet_information(true, ipv6) {
                (&mut local_buf[..self.offset]).put_slice(header.as_ref());
                (&mut local_buf[self.offset..in_buf_len]).put_slice(in_buf);
                local_buf
            } else {
                in_buf
            }
        } else {
            in_buf
        };
        let amount = self.fd.write(either_buf)?;
        Ok(amount - self.offset)
    }

    pub(crate) fn recv(&self, mut in_buf: &mut [u8]) -> io::Result<usize> {
        const STACK_BUF_LEN: usize = crate::DEFAULT_MTU as usize + PIL;
        let in_buf_len = in_buf.len() + self.offset;

        // The following logic is to prevent dynamically allocating Vec on every recv
        // As long as the MTU is set to value lesser than 1500, this api uses `stack_buf`
        // and avoids `Vec` allocation

        let local_buf_v0 =
            local_buf_util!(in_buf_len > STACK_BUF_LEN && self.offset != 0, in_buf_len);
        need_mut! {local_buf_v1,local_buf_v0};
        #[allow(clippy::useless_asref)]
        let local_buf = local_buf_v1.as_mut();

        let either_buf = if self.offset != 0 {
            &mut *local_buf
        } else {
            &mut *in_buf
        };
        let amount = self.fd.read(either_buf)?;
        if self.offset != 0 {
            in_buf.put_slice(&local_buf[self.offset..amount]);
        }
        Ok(amount - self.offset)
    }
}

impl Read for Tun {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv(buf)
    }
}

impl Write for Tun {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for Tun {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for Tun {
    fn into_raw_fd(self) -> RawFd {
        self.fd.as_raw_fd()
    }
}