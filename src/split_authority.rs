use crate::sqlite_authority::SqliteAuthority;
use hickory_server::authority::{
    Authority, LookupError, LookupObject, LookupOptions, LookupRecords, MessageRequest,
    UpdateResult, ZoneType,
};
use hickory_server::proto::op::ResponseCode;
use hickory_server::proto::rr::{LowerName, Record, RecordType};
use hickory_server::resolver::lookup::Lookup;
use hickory_server::server::RequestInfo;
use hickory_server::store::forwarder::ForwardAuthority;
use hickory_server::store::in_memory::InMemoryAuthority;
use std::io;

pub struct SplitAuthority {
    pub origin: LowerName,
    pub in_memory_authority: InMemoryAuthority,
    pub sqlite_authority: SqliteAuthority,
    pub forward_authority: ForwardAuthority,
}

pub struct SplitLookup {
    pub auth_lookup: Option<LookupRecords>,
    pub lookup: Option<Lookup>,
}

impl SplitLookup {}

impl LookupObject for SplitLookup {
    fn is_empty(&self) -> bool {
        if let Some(auth_lookup) = &self.auth_lookup {
            return auth_lookup.is_empty();
        }
        if let Some(lookup) = &self.lookup {
            return lookup.is_empty();
        }
        panic!("No delegate");
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Record> + Send + 'a> {
        if let Some(auth_lookup) = &self.auth_lookup {
            return Box::new(auth_lookup.iter());
        }
        if let Some(lookup) = &self.lookup {
            return Box::new(lookup.record_iter());
        }
        panic!("No delegate");
    }

    fn take_additionals(&mut self) -> Option<Box<dyn LookupObject>> {
        None
    }
}

#[async_trait::async_trait]
impl Authority for SplitAuthority {
    type Lookup = SplitLookup;

    fn zone_type(&self) -> ZoneType {
        ZoneType::Forward
    }

    fn is_axfr_allowed(&self) -> bool {
        false
    }

    async fn update(&self, _update: &MessageRequest) -> UpdateResult<bool> {
        Err(ResponseCode::NotImp)
    }

    fn origin(&self) -> &LowerName {
        &self.origin
    }

    async fn lookup(
        &self,
        name: &LowerName,
        rtype: RecordType,
        lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        let in_memory_resolve = self
            .in_memory_authority
            .lookup(name, rtype, lookup_options)
            .await;
        if in_memory_resolve.is_ok() {
            return in_memory_resolve.map(|l| SplitLookup {
                auth_lookup: Some(l.unwrap_records()),
                lookup: None,
            });
        }
        let db_resolve = self
            .sqlite_authority
            .lookup(name, rtype, lookup_options)
            .await;
        if db_resolve.is_ok() {
            return db_resolve.map(|l| SplitLookup {
                auth_lookup: None,
                lookup: Some(l),
            });
        }
        let forward_resolve = self
            .forward_authority
            .lookup(name, rtype, lookup_options)
            .await;
        return forward_resolve.map(|l| SplitLookup {
            auth_lookup: None,
            lookup: Some(l.0),
        });
    }

    async fn search(
        &self,
        request: RequestInfo<'_>,
        lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        self.lookup(
            request.query.name(),
            request.query.query_type(),
            lookup_options,
        )
        .await
    }

    async fn get_nsec_records(
        &self,
        _name: &LowerName,
        _lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        Err(LookupError::from(io::Error::new(
            io::ErrorKind::Other,
            "Getting NSEC records is unimplemented",
        )))
    }
}
