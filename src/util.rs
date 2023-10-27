use anyhow::{anyhow, Result};
use hickory_server::proto::rr::rdata::{A, AAAA};
use hickory_server::proto::rr::{RData, RecordType};
use hickory_server::resolver::Name;
use local_ip_address::{list_afinet_netifas, local_ip, local_ipv6};
use rusqlite_migration::{Migrations, M};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio_rusqlite::Connection;
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;

pub fn get_iface_addr(iface: Option<String>, protocol: Option<String>) -> Result<IpAddr> {
    debug!("Searching for interface {:?}", iface);
    let desired_protocol = protocol.unwrap_or("ipv4".to_string());
    if iface.is_none() {
        return if desired_protocol.eq_ignore_ascii_case("ipv4") {
            Ok(local_ip()?)
        } else if desired_protocol.eq_ignore_ascii_case("ipv6") {
            Ok(local_ipv6()?)
        } else {
            Err(anyhow!("Unknown protocol {:?}", desired_protocol))
        };
    }
    let iface = iface.unwrap();
    let interfaces = list_afinet_netifas().unwrap();
    for (system_iface, system_ip) in interfaces.into_iter() {
        debug!("Found interface {:?} with ip {:?}", system_iface, system_ip);
        let system_protocol = if system_ip.is_ipv6() {
            "ipv6"
        } else if system_ip.is_ipv4() {
            "ipv4"
        } else {
            "unknown"
        };
        if !system_protocol.eq_ignore_ascii_case(desired_protocol.as_str()) {
            continue;
        }
        if !system_iface.eq_ignore_ascii_case(iface.as_str()) {
            continue;
        }
        debug!("Found ip {:?} for interface {:?}", system_ip, iface);
        return Ok(system_ip);
    }
    return Err(anyhow!("Interface not found"));
}

pub fn get_socket_addr(
    iface: Option<String>,
    port: u16,
    proto: Option<String>,
) -> Result<SocketAddr> {
    let ip_addr = if iface.is_some() {
        get_iface_addr(iface, proto)?
    } else {
        let proto = proto.unwrap_or("ipv4".to_string());
        let ip_str = match proto.as_str() {
            "ipv4" => Ok("0.0.0.0"),
            "ipv6" => Ok("::"),
            _ => Err(anyhow!("Unknown protocol {:?}", proto)),
        }?;
        IpAddr::from_str(ip_str)?
    };
    Ok(SocketAddr::new(ip_addr, port))
}

pub fn configure_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

pub async fn open_database(data_dir: &PathBuf, db_filename: &PathBuf) -> Result<Connection> {
    let db_path = data_dir.join(db_filename);
    info!("Opening database at {:?}", db_path);
    fs::create_dir_all(data_dir).await?;

    let conn = Connection::open(db_path).await?;
    Ok(conn)
}

pub async fn migrate_database(conn: Arc<Connection>) -> Result<()> {
    let migrations = Migrations::new(vec![M::up(
        r#"
            CREATE TABLE records(
                name VARCHAR(256) NOT NULL,
                type VARCHAR(16) NOT NULL,
                data VARCHAR(512),
                ttl INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (name, type)
            );
        "#,
    )
    .down("DROP TABLE records;")]);
    conn.call(move |mut conn| {
        info!("Migrating database to latest");
        conn.pragma_update(None, "journal_mode", &"WAL").unwrap();
        migrations.to_latest(&mut conn).unwrap();
        Ok(())
    })
    .await?;
    Ok(())
}

pub fn get_ip_addr_record_type(ip_addr: &IpAddr) -> Result<RecordType> {
    return if ip_addr.is_ipv4() {
        Ok(RecordType::A)
    } else if ip_addr.is_ipv6() {
        Ok(RecordType::AAAA)
    } else {
        Err(anyhow!("{:?} is not an IPv4 or IPv6", ip_addr))
    };
}

pub fn create_record_data(value: &str) -> Result<Option<RData>> {
    let ip_addr: IpAddr = value.parse()?;
    let record_type = get_ip_addr_record_type(&ip_addr)?;
    return match record_type {
        RecordType::A => Ok(Some(RData::A(A::from_str(value).unwrap()))),
        RecordType::AAAA => Ok(Some(RData::AAAA(AAAA::from_str(value).unwrap()))),
        _ => Err(anyhow!("Unsupported record type {:?}", record_type)),
    };
}

pub fn render_record_name(key: &String, zone_name: &Name) -> Result<Name> {
    return if "@".eq(key.as_str()) {
        Ok(zone_name.clone())
    } else {
        let name = Name::from_str(format!("{0}.{1}", key, zone_name).as_str())?;
        Ok(name)
    };
}

pub fn parse_ip_optional_socket(value: &str, default_port: u16) -> Result<SocketAddr> {
    if let Ok(socket_addr) = value.parse() {
        Ok(socket_addr)
    } else {
        let ip_addr: IpAddr = value.parse()?;
        Ok(SocketAddr::new(ip_addr, default_port))
    }
}
