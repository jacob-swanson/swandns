use crate::proto::FindUniqueRecordRequest;
use crate::record_repository::RecordRepository;
use crate::util::create_record_data;
use hickory_server::authority::{
    Authority, LookupError, LookupOptions, MessageRequest, UpdateResult, ZoneType,
};
use hickory_server::proto::op::{Query, ResponseCode};
use hickory_server::proto::rr::{DNSClass, LowerName, Record, RecordType};
use hickory_server::resolver::lookup::Lookup;
use hickory_server::resolver::{IntoName, Name};
use hickory_server::server::RequestInfo;
use std::io;
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use time::Duration;

pub struct SqliteAuthority {
    pub origin: LowerName,
    pub zone_type: ZoneType,
    pub repo: Arc<RecordRepository>,
}

#[async_trait::async_trait]
impl Authority for SqliteAuthority {
    type Lookup = Lookup;

    fn zone_type(&self) -> ZoneType {
        self.zone_type
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
        _lookup_options: LookupOptions,
    ) -> Result<Self::Lookup, LookupError> {
        let mut name_param = name.to_string();
        name_param = name_param.strip_suffix(".").unwrap().to_string();
        let records_result = self
            .repo
            .find_unique(FindUniqueRecordRequest {
                name: name_param,
                r#type: rtype.to_string(),
            })
            .await;

        if records_result.is_err() {
            return Err(LookupError::ResponseCode(ResponseCode::NXDomain));
        }

        // TODO: Support more than one record?
        let db_record = records_result.unwrap();
        let mut dns_record = Record::new();
        dns_record
            .set_name(Name::from_str(db_record.name.as_str()).unwrap())
            .set_rr_type(RecordType::from_str(db_record.r#type.as_str()).unwrap())
            .set_dns_class(DNSClass::IN)
            .set_ttl(db_record.ttl)
            .set_data(create_record_data(db_record.data.as_str()).unwrap());
        let query = Query::query(name.into_name().unwrap(), rtype);
        let ttl = Instant::now().add(Duration::seconds(30));
        Ok(Lookup::new_with_deadline(
            query,
            Arc::new([dns_record.clone()]),
            ttl,
        ))
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
