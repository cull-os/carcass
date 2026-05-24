use std::{
   io,
   iter,
   net,
};

use super::address;

pub const MTU: u16 = 1420;

const LOCAL: net::Ipv6Addr = net::Ipv6Addr::new(0xFE80, 0, 0, 0, 0, 0, 0, 0x67);

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to create tun device")]
   CreateDevice(#[source] io::Error),

   #[cfg(target_os = "macos")]
   #[error("failed to read the tun device name")]
   DeviceName(#[source] io::Error),

   #[cfg(target_os = "macos")]
   #[error("failed to enumerate interface addresses")]
   ListAddresses(#[source] io::Error),

   #[cfg(target_os = "macos")]
   #[error("failed to remove link-local '{address}' from the tun")]
   RemoveLinkLocal {
      address: net::Ipv6Addr,
      #[source]
      source:  io::Error,
   },

   #[error("failed to create route manager")]
   RouteManager(#[source] io::Error),

   #[error("failed to add '{destination}/{prefix_length}' tun route")]
   TunRoute {
      destination:   net::Ipv6Addr,
      prefix_length: u8,
      #[source]
      source:        io::Error,
   },

   #[error("failed to look up loopback interface")]
   LoopbackInterface(#[source] io::Error),

   #[error("failed to assign '{address}' to loopback")]
   Loopback {
      address: net::Ipv6Addr,
      #[source]
      source:  io::Error,
   },
}

pub struct Interface {
   device: tun_rs::AsyncDevice,
}

impl Interface {
   #[cfg_attr(target_os = "macos", expect(clippy::cognitive_complexity))]
   pub async fn create(name: Option<&str>, host_prefix: address::Prefix) -> Result<Self, Error> {
      let mut builder = tun_rs::DeviceBuilder::new().mtu(MTU);

      if let Some(name) = name {
         builder = builder.name(name);
      }

      let device = builder
         .ipv6(LOCAL, 16)
         .associate_route(false)
         .build_async()
         .map_err(Error::CreateDevice)?;

      #[cfg(target_os = "macos")]
      {
         use nix::ifaddrs as nix_ifaddrs;

         let name = device.name().map_err(Error::DeviceName)?;

         let strays = nix_ifaddrs::getifaddrs()
            .map_err(|error| Error::ListAddresses(io::Error::from(error)))?
            .filter_map(|interface| {
               if interface.interface_name != name {
                  return None;
               }

               let address = interface.address?.as_sockaddr_in6()?.ip();

               if address == LOCAL {
                  return None;
               }

               Some(address)
            });

         for address in strays {
            tracing::info!(%address, "Removing stray link-local from tun");
            device
               .remove_address(net::IpAddr::from(address))
               .map_err(|source| Error::RemoveLinkLocal { address, source })?;
         }
      }

      {
         let mut route = route::Manager::new().map_err(Error::RouteManager)?;

         for (&destination, prefix_length) in [net::Ipv6Addr::from(address::Prefix::LOCAL)]
            .iter()
            .zip(iter::repeat(
               u8::try_from(address::VPN_PREFIX.len() * 8).expect("prefix must fit in u8"),
            ))
         {
            tracing::info!(%destination, prefix_length, "Adding tun route");
            route
               .add(&device)
               .destination(destination)
               .prefix_length(prefix_length)
               .await
               .map_err(|source| {
                  Error::TunRoute {
                     destination,
                     prefix_length,
                     source,
                  }
               })?;
         }
      }

      {
         let mut loopback = loopback::Manager::new().map_err(Error::LoopbackInterface)?;

         for (&address, prefix_length) in
            [host_prefix.host_addr(), address::Prefix::LOCAL.host_addr()]
               .iter()
               .zip(iter::repeat(
                  u8::try_from(address::HOST_PREFIX_RANGE.end * 8)
                     .expect("prefix length must fit in u8"),
               ))
         {
            tracing::info!(%address, prefix_length, "Assigning prefix to loopback");
            loopback
               .assign(address, prefix_length)
               .await
               .map_err(|source| Error::Loopback { address, source })?;
         }
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

mod route {
   use std::{
      io,
      net,
   };

   use tokio::task;

   pub struct Manager {
      #[expect(dead_code, reason = "owned for Drop")]
      join_set: task::JoinSet<()>,

      #[cfg(target_os = "linux")]
      handle: rtnetlink::Handle,
      #[cfg(target_os = "macos")]
      handle: route_manager::AsyncRouteManager,
   }

   #[bon::bon]
   impl Manager {
      pub fn new() -> io::Result<Self> {
         #[cfg_attr(
            target_os = "macos",
            expect(unused_mut, reason = "macos has no tasks to spawn into the JoinSet")
         )]
         let mut join_set = task::JoinSet::new();

         Ok(Self {
            handle: cfg_select! {
               target_os = "linux" => {{
                  let (connection, handle, _) = rtnetlink::new_connection()?;
                  join_set.spawn(connection);
                  handle
               }}

               target_os = "macos" => {
                  route_manager::AsyncRouteManager::new()?
               }

               _ => {
                  compile_error!("unsupported platform")
               }
            },
            join_set,
         })
      }

      #[builder(finish_fn(name = "prefix_length"))]
      pub async fn add(
         &mut self,
         #[builder(start_fn)] device: &tun_rs::AsyncDevice,
         #[builder(finish_fn)] prefix_length: u8,
         destination: net::Ipv6Addr,
      ) -> io::Result<()> {
         let identifier = device.if_index()?;

         cfg_select! {
            target_os = "linux" => {
               self
                  .handle
                  .route()
                  .add()
                  .v6()
                  .destination_prefix(destination, prefix_length)
                  .output_interface(identifier)
                  .execute()
                  .await
                  .map_err(io::Error::other)?;
            }

            target_os = "macos" => {
               self
                  .handle
                  .add(
                     &route_manager::Route::new(net::IpAddr::from(destination), prefix_length)
                        .with_if_index(identifier),
                  )
                  .await?;
            }

            _ => {
               compile_error!("unsupported platform");
            }
         }

         Ok(())
      }
   }
}

mod loopback {
   use std::{
      io,
      net,
   };

   use tokio::task;

   pub struct Manager {
      #[expect(dead_code, reason = "owned for Drop")]
      join_set: task::JoinSet<()>,

      #[cfg(target_os = "linux")]
      handle: rtnetlink::Handle,
      #[cfg(target_os = "macos")]
      handle: route_manager::AsyncRouteManager,

      #[cfg(target_os = "linux")]
      identifier: u32,
      #[cfg(target_os = "macos")]
      identifier: String,
   }

   impl Manager {
      pub fn new() -> io::Result<Self> {
         #[cfg_attr(target_os = "macos", expect(unused_mut))]
         let mut join_set = task::JoinSet::new();

         Ok(Self {
            handle: cfg_select! {
               target_os = "linux" => {{
                  let (connection, handle, _) = rtnetlink::new_connection()?;
                  join_set.spawn(connection);
                  handle
               }}

               target_os = "macos" => {
                  route_manager::AsyncRouteManager::new()?
               }

               _ => {
                  compile_error!("unsupported platform")
               },
            },
            identifier: {
               use nix::{
                  ifaddrs as nix_ifaddrs,
                  net::if_ as nix_if,
               };

               let name = nix_ifaddrs::getifaddrs()?
                  .find(|interface| {
                     interface
                        .flags
                        .contains(nix_if::InterfaceFlags::IFF_LOOPBACK)
                  })
                  .map(|interface| interface.interface_name)
                  .ok_or_else(|| {
                     io::Error::new(io::ErrorKind::NotFound, "no loopback interface found")
                  })?;

               cfg_select! {
                  target_os = "linux" => {
                     nix_if::if_nametoindex(name.as_str())?
                  }

                  target_os = "macos" => {
                     name
                  }

                  _ => {
                     compile_error!("unsupported platform")
                  }
               }
            },
            join_set,
         })
      }

      #[cfg_attr(target_os = "macos", expect(clippy::items_after_statements))]
      pub async fn assign(&mut self, address: net::Ipv6Addr, prefix_length: u8) -> io::Result<()> {
         assert!(
            prefix_length.is_multiple_of(8),
            "prefix length must be byte-aligned",
         );

         cfg_select! {
            target_os = "linux" => {
               self
                  .handle
                  .address()
                  .add(self.identifier, net::IpAddr::from(address), prefix_length)
                  .execute()
                  .await
                  .map_err(io::Error::other)?;
            }

            target_os = "macos" => {
               use std::{
                  array,
                  mem,
                  os::fd::AsRawFd as _,
               };

               use nix::sys::socket as nix_socket;

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
                     sin6_family:   u8::try_from(libc::AF_INET6)
                        .expect("AF_INET6 must fit in u8"),
                     sin6_port:     0,
                     sin6_flowinfo: 0,
                     sin6_addr:     libc::in6_addr {
                        s6_addr: address.octets(),
                     },
                     sin6_scope_id: 0,
                  }
               }

               let socket = nix_socket::socket(
                  nix_socket::AddressFamily::Inet6,
                  nix_socket::SockType::Datagram,
                  nix_socket::SockFlag::empty(),
                  None,
               )?;

               let request = InterfaceAliasRequest {
                  name:             {
                     let mut name = [0; _];
                     for (destination, &source) in name.iter_mut().zip(self.identifier.as_bytes()) {
                        *destination = source.cast_signed();
                     }
                     name
                  },
                  address:          sockaddr_in6(address),
                  destination:      {
                     /// Zeroed `sockaddr_in6` with `AF_UNSPEC` family. XNU rejects
                     /// `AF_INET6` destinations when prefix length is not /128.
                     const SOCKADDR_IN6_UNSPEC: libc::sockaddr_in6 = libc::sockaddr_in6 {
                        sin6_len:      0,
                        sin6_family:   0,
                        sin6_port:     0,
                        sin6_flowinfo: 0,
                        sin6_addr:     libc::in6_addr { s6_addr: [0; 16] },
                        sin6_scope_id: 0,
                     };

                     SOCKADDR_IN6_UNSPEC
                  },
                  prefix_mask:      sockaddr_in6(net::Ipv6Addr::from(array::from_fn(|index| {
                     if index < usize::from(prefix_length / 8) {
                        0xFF_u8
                     } else {
                        0
                     }
                  }))),
                  flags:            0,
                  address_lifetime: AddressLifetime {
                     expire:             0,
                     preferred:          0,
                     valid_lifetime:     u32::MAX,
                     preferred_lifetime: u32::MAX,
                  },
               };

               // SAFETY: request is a valid InterfaceAliasRequest and socket is an open fd.
               unsafe { add_ipv6_address(socket.as_raw_fd(), &raw const request) }?;

               // XNU's in6_ifaddloop auto-installs this /128 host route on lo0,
               // but rt_ifa gets stamped with the tun's ifaddr (fd67::/16 wins
               // the radix lookup at insert time). Source selection then follows
               // rt_ifa->ifa_ifp and picks the tun's link-local. Re-adding via
               // -interface lo0 re-stamps rt_ifa to the loopback ifaddr.
               let route = route_manager::Route::new(
                  net::IpAddr::from(address),
                  u8::try_from(u128::BITS).expect("u128::BITS must fit in u8"),
               )
               .with_if_name(self.identifier.clone());
               self.handle.delete(&route).await?;
               self.handle.add(&route).await?;
            }

            _ => {
               compile_error!("unsupported platform");
            }
         }

         Ok(())
      }
   }
}
