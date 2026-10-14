use crate::app::model::{Model, logs::LogEntry};

pub fn parse_message(message: Vec<u8>, model: &mut Model) {
    log::info!("received {}B", message.len());
    match serde_cbor_2::from_slice::<proto::Message>(&message) {
        Ok(message) => {
            match message {
                proto::Message::Value(value) => {
                    model.streams.append(&value);
                }
                proto::Message::ValueMetadata(metadata) => {
                    model.streams.set_unit(&metadata.path, &metadata.unit);
                }
                proto::Message::Log(record) => {
                    log::info!("received log message");
                    model.logs.push(LogEntry::from_proto(&record));
                }
            };
        }
        Err(e) => {
            log::warn!("error parsing message: {}", e);
        }
    }
}
