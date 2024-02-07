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

use crate::platform::linux::offload::GroTable;
use bytes::BytesMut;
use etherparse::{IpHeaders, UdpHeader};
use std::net::IpAddr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UdpFlowKey {
    pub(crate) src_addr: IpAddr,
    pub(crate) dst_addr: IpAddr,
    pub(crate) src_port: u16,
    pub(crate) dst_port: u16,
    pub(crate) recv_ack: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct UdpGroItem {
    pub(crate) num_merged: u32,
    pub(crate) ip_header: IpHeaders,
    pub(crate) udp_header: UdpHeader,
    pub(crate) checksum_known_invalid: bool,
    pub(crate) data: BytesMut,
}

pub(crate) type UdpGroTable = GroTable<UdpFlowKey, UdpGroItem>;
