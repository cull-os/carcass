use std::{
   io,
   net,
};

use super::address;

pub const MTU: u16 = 1420;

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to create tun device")]
   CreateDevice(#[source] io::Error),

   #[cfg(target_os = "linux")]
   #[error("failed to create netlink connection")]
   CreateNetlinkConnection(#[source] io::Error),

   #[cfg_attr(target_os = "linux", error("failed to get tun interface index"))]
   #[cfg_attr(target_os = "macos", error("failed to find loopback interface"))]
   GetInterface(#[source] io::Error),

   #[cfg(target_os = "linux")]
   #[error("failed to add local route")]
   AddLocalRoute(#[source] rtnetlink::Error),

   #[cfg(target_os = "macos")]
   #[error("failed to add local route")]
   AddLocalRoute(#[source] io::Error),
}

pub struct Interface {
   device: tun_rs::AsyncDevice,
}

impl Interface {
   #[cfg_attr(
      not(target_os = "linux"),
      expect(
         clippy::unused_async,
         reason = "await used on linux for netlink route addition"
      )
   )]
   pub async fn create(name: Option<&str>, host_prefix: address::Prefix) -> Result<Self, Error> {
      let prefix_length =
         u8::try_from(address::HOST_PREFIX_RANGE.end * 8).expect("prefix length must fit in u8");

      let host_prefix = net::Ipv6Addr::from({
         let mut octets = [0; _];
         octets[..host_prefix.len()].copy_from_slice(&*host_prefix);
         *octets.last_mut().expect("address array must not be empty") = 1;
         octets
      });

      let zero_prefix = net::Ipv6Addr::from({
         let mut octets = [0; _];
         octets[address::VPN_PREFIX_RANGE].copy_from_slice(&address::VPN_PREFIX);
         *octets.last_mut().expect("address array must not be empty") = 1;
         octets
      });

      let local_prefixes = [host_prefix, zero_prefix];

      let mut builder = tun_rs::DeviceBuilder::new()
         .ipv6(
            host_prefix,
            u8::try_from(address::VPN_PREFIX.len() * 8).expect("prefix must fit in u8"),
         )
         .mtu(MTU);

      if let Some(name) = name {
         builder = builder.name(name);
      }

      let device = builder.build_async().map_err(Error::CreateDevice)?;

      cfg_select! {
         target_os = "macos" => {
            let loopback = loopback::interface_name().map_err(Error::GetInterface)?;
            for &address in &local_prefixes {
               tracing::info!(%address, prefix_length, loopback, "Assigning prefix to loopback");
               loopback::assign(&loopback)
                  .address(address)
                  .prefix_length(prefix_length)
                  .map_err(Error::AddLocalRoute)?;
            }
         }
         target_os = "linux" => {
            const RT_TABLE_LOCAL: u32 = 255;

            use rtnetlink::packet_route::route;

            let (connection, handle, _) =
               rtnetlink::new_connection().map_err(Error::CreateNetlinkConnection)?;
            tokio::spawn(connection);

            let interface_index = device.if_index().map_err(Error::GetInterface)?;

            for &address in &local_prefixes {
               handle
                  .route()
                  .add(
                     rtnetlink::RouteMessageBuilder::<net::Ipv6Addr>::new()
                        .destination_prefix(address, prefix_length)
                        .output_interface(interface_index)
                        .kind(route::RouteType::Local)
                        .scope(route::RouteScope::Host)
                        .table_id(RT_TABLE_LOCAL)
                        .build(),
                  )
                  .execute()
                  .await
                  .map_err(Error::AddLocalRoute)?;
            }
         }
         _ => compile_error!("unsupported platform")
      }

      Ok(Self { device })
   }

   pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
      self.device.recv(buf).await
   }

   pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
      self.device.send(buf).await
   }
}

#[cfg(target_os = "macos")]
mod loopback {
   use std::{
      array,
      io,
      mem,
      net,
      os::fd::AsRawFd as _,
   };

   use nix::{
      ifaddrs as nix_ifaddrs,
      net::if_ as nix_if,
      sys::socket as nix_socket,
   };

   /// Interface address flag that makes the kernel skip NDP duplicate address
   /// detection, which would otherwise send neighbor solicitations on the wire
   /// and delay address availability by ~1s. Pointless on loopback.
   const IN6_IFF_NODAD: libc::c_int = 0x0020;

   /// Zeroed `sockaddr_in6` with `AF_UNSPEC` family. XNU rejects `AF_INET6`
   /// destinations on loopback when prefix length is not /128.
   const SOCKADDR_IN6_UNSPEC: libc::sockaddr_in6 = libc::sockaddr_in6 {
      sin6_len:      0,
      sin6_family:   0,
      sin6_port:     0,
      sin6_flowinfo: 0,
      sin6_addr:     libc::in6_addr { s6_addr: [0; 16] },
      sin6_scope_id: 0,
   };

   #[repr(C)]
   #[derive(Clone)]
   struct AddressLifetime {
      expire:             libc::time_t,
      preferred:          libc::time_t,
      valid_lifetime:     u32,
      preferred_lifetime: u32,
   }

   #[repr(C)]
   #[derive(Clone)]
   struct InterfaceAliasRequest {
      name:             [libc::c_char; libc::IFNAMSIZ],
      address:          libc::sockaddr_in6,
      destination:      libc::sockaddr_in6,
      prefix_mask:      libc::sockaddr_in6,
      flags:            libc::c_int,
      address_lifetime: AddressLifetime,
   }

   nix::ioctl_write_ptr!(add_ipv6_address, b'i', 26, InterfaceAliasRequest);

   fn sockaddr_in6(address: net::Ipv6Addr) -> libc::sockaddr_in6 {
      libc::sockaddr_in6 {
         sin6_len:      u8::try_from(mem::size_of::<libc::sockaddr_in6>())
            .expect("libc::sockaddr_in6 size must fit in u8"),
         sin6_family:   u8::try_from(libc::AF_INET6).expect("AF_INET6 must fit in u8"),
         sin6_port:     0,
         sin6_flowinfo: 0,
         sin6_addr:     libc::in6_addr {
            s6_addr: address.octets(),
         },
         sin6_scope_id: 0,
      }
   }

   pub fn interface_name() -> io::Result<String> {
      nix_ifaddrs::getifaddrs()
         .map_err(io::Error::from)?
         .find(|interface| {
            interface
               .flags
               .contains(nix_if::InterfaceFlags::IFF_LOOPBACK)
         })
         .map(|interface| interface.interface_name)
         .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no loopback interface found"))
   }

   #[bon::builder(finish_fn(name = "prefix_length"))]
   pub fn assign(
      #[builder(start_fn)] loopback: &str,
      #[builder(finish_fn)] prefix_length: u8,
      address: net::Ipv6Addr,
   ) -> io::Result<()> {
      assert!(
         prefix_length.is_multiple_of(8),
         "prefix length must be byte-aligned",
      );

      let socket = nix_socket::socket(
         nix_socket::AddressFamily::Inet6,
         nix_socket::SockType::Datagram,
         nix_socket::SockFlag::empty(),
         None,
      )
      .map_err(io::Error::from)?;

      let request = InterfaceAliasRequest {
         name:             {
            let mut name = [0; _];
            for (destination, &source) in name.iter_mut().zip(loopback.as_bytes()) {
               *destination = source.cast_signed();
            }
            name
         },
         address:          sockaddr_in6(address),
         destination:      SOCKADDR_IN6_UNSPEC,
         prefix_mask:      sockaddr_in6(net::Ipv6Addr::from(array::from_fn(|index| {
            if index < usize::from(prefix_length / 8) {
               0xFF_u8
            } else {
               0
            }
         }))),
         flags:            IN6_IFF_NODAD,
         address_lifetime: AddressLifetime {
            expire:             0,
            preferred:          0,
            valid_lifetime:     u32::MAX,
            preferred_lifetime: u32::MAX,
         },
      };

      // SAFETY: request is a valid InterfaceAliasRequest and socket is an open fd.
      unsafe { add_ipv6_address(socket.as_raw_fd(), &raw const request) }
         .map_err(io::Error::from)?;

      Ok(())
   }
}
