use super::declaration::DatabaseDecl;
use super::engine::DatabaseEngine;
use sha2::{Digest, Sha256};

const FINGERPRINT_VERSION: &[u8] = b"yssbi.database-declaration.fingerprint.v2";

pub(super) fn fingerprint_declaration(declaration: &DatabaseDecl) -> [u8; 32] {
    let mut encoding = Vec::new();
    write_bytes(&mut encoding, FINGERPRINT_VERSION);
    write_field(&mut encoding, 0x01, |field| {
        write_bytes(field, declaration.id.as_str().as_bytes());
    });
    write_field(&mut encoding, 0x02, |field| {
        encode_engine(field, &declaration.engine)
    });
    write_field(&mut encoding, 0x03, |field| {
        write_u32(field, declaration.schema_version);
    });
    write_field(&mut encoding, 0x04, |field| {
        field.push(u8::from(declaration.required));
    });
    write_field(&mut encoding, 0x05, |field| {
        write_bytes(field, declaration.name.as_bytes());
    });
    let digest = Sha256::digest(&encoding);
    let mut fingerprint = [0; 32];
    fingerprint.copy_from_slice(&digest);
    fingerprint
}

fn encode_engine(output: &mut Vec<u8>, engine: &DatabaseEngine) {
    match engine {
        DatabaseEngine::Dataset {} => output.push(0x07),
    }
}

fn write_field(output: &mut Vec<u8>, tag: u8, encode: impl FnOnce(&mut Vec<u8>)) {
    let mut value = Vec::new();
    encode(&mut value);
    output.push(tag);
    write_u64(output, value.len() as u64);
    output.extend_from_slice(&value);
}

fn write_bytes(output: &mut Vec<u8>, value: &[u8]) {
    write_u64(output, value.len() as u64);
    output.extend_from_slice(value);
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn write_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::fingerprint_declaration;
    use crate::{DatabaseDecl, DatabaseEngine, DatabaseId};

    #[test]
    fn declaration_fingerprint_has_a_stable_dataset_storage_digest() {
        let declaration = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::Dataset {},
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };

        assert_eq!(
            fingerprint_declaration(&declaration),
            [
                0x18, 0xa1, 0x29, 0x39, 0x8f, 0x4b, 0x6b, 0x1a, 0x0e, 0x4b, 0xd7, 0x4d, 0xf5, 0x42,
                0x11, 0x08, 0x53, 0x76, 0x6e, 0x17, 0x9e, 0x72, 0x6d, 0xe2, 0xf4, 0x6c, 0x19, 0x6b,
                0x4e, 0x6d, 0xe7, 0x94
            ]
        );
    }
}
