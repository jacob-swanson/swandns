use crate::record_repository::RecordRepository;
use crate::split_authority::SplitAuthority;
use crate::sqlite_authority::SqliteAuthority;
use crate::util::{
    create_record_data, get_ip_addr_record_type, get_socket_addr, parse_ip_optional_socket,
    render_record_name,
};
use crate::ServerConfig;
use anyhow::Result;
use hickory_server::authority::{Catalog, ZoneType};
use hickory_server::proto::rr::{DNSClass, LowerName, Record};
use hickory_server::recursor::NameServerConfig;
use hickory_server::resolver::config::{NameServerConfigGroup, Protocol};
use hickory_server::resolver::Name;
use hickory_server::store::forwarder::{ForwardAuthority, ForwardConfig};
use hickory_server::store::in_memory::InMemoryAuthority;
use hickory_server::ServerFuture;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};
use tracing::info;

pub struct DnsServer {
    pub repo: Arc<RecordRepository>,
    pub cfg: Arc<ServerConfig>,
}

impl DnsServer {
    pub fn get_socket_addr(&self) -> Result<SocketAddr> {
        let socket_addr = get_socket_addr(self.cfg.bind.clone(), self.cfg.dns_port, None)?;
        Ok(socket_addr)
    }

    pub async fn run(&self) -> Result<()> {
        let mut catalog = Catalog::new();

        // Zones
        for zone_config in self.cfg.zones.clone().into_iter() {
            let zone_name = Name::from_str(zone_config.name.as_str())?;

            // In-memory authority for static records.
            let in_memory_authority =
                InMemoryAuthority::empty(zone_name.clone(), ZoneType::Primary, false);
            let mut i: u32 = 0;
            for record_config in zone_config.records.into_iter() {
                let name = render_record_name(&record_config.key, &zone_name)?;
                let value = record_config.value;
                let ip_addr: IpAddr = value.parse()?;
                let rr_type = get_ip_addr_record_type(&ip_addr)?;
                info!(
                    "Registering record {:?}={:?} for zone {:?}",
                    name, value, zone_name
                );
                let mut record = Record::new();
                let rdata = create_record_data(value.as_str())?;
                record
                    .set_name(name)
                    .set_rr_type(rr_type)
                    .set_dns_class(DNSClass::IN)
                    .set_ttl(30)
                    .set_data(rdata);
                in_memory_authority.upsert(record, i).await;
                i += 1;
            }

            // Forwarding authority
            let mut nameservers = NameServerConfigGroup::new();
            for nameserver in self.cfg.nameservers.iter() {
                let socket_addr = parse_ip_optional_socket(nameserver, 53)?;
                info!(
                    "Registering upstream {:?} for zone {:?}",
                    socket_addr, zone_name
                );
                nameservers.push(NameServerConfig::new(socket_addr, Protocol::Udp));
                nameservers.push(NameServerConfig::new(socket_addr, Protocol::Tcp));
            }
            let forward_config = ForwardConfig {
                name_servers: nameservers,
                options: None,
            };
            let forward_authority = ForwardAuthority::try_from_config(
                zone_name.clone(),
                ZoneType::Forward,
                &forward_config,
            )
            .unwrap();

            // Sqlite authority
            let sqlite_authority = SqliteAuthority {
                origin: LowerName::from(zone_name.clone()),
                zone_type: ZoneType::Primary,
                repo: self.repo.clone(),
            };

            // Split authority
            let split_authority = SplitAuthority {
                origin: LowerName::from(zone_name.clone()),
                in_memory_authority,
                sqlite_authority,
                forward_authority,
            };

            catalog.upsert(
                LowerName::from(zone_name.clone()),
                Box::new(Arc::new(split_authority)),
            );
        }

        let mut server = ServerFuture::new(catalog);

        // Configure UDP listener
        let dns_listen_addr = self.get_socket_addr()?;
        let dns_udp_socket = UdpSocket::bind(dns_listen_addr).await?;
        let dns_upd_local_addr = dns_udp_socket.local_addr()?;
        server.register_socket(dns_udp_socket);
        info!("DNS server listening on {:?} (udp)", dns_upd_local_addr);

        // Configure TCP listener
        let dns_tcp_listener = TcpListener::bind(dns_listen_addr).await?;
        let dns_tcp_request_timeout = Duration::from_secs(3);
        let dns_tpc_local_addr = dns_tcp_listener.local_addr()?;
        server.register_listener(dns_tcp_listener, dns_tcp_request_timeout);
        info!("DNS server listening on {:?} (tcp)", dns_tpc_local_addr);

        server.block_until_done().await?;
        Ok(())
    }
}
