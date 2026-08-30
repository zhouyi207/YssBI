use super::declaration::DatabaseDecl;
use super::engine::{DatabaseEngine, DatabaseEngineSql};
use sha2::{Digest, Sha256};

const FINGERPRINT_VERSION: &[u8] = b"yssbi.database-declaration.fingerprint.v1";

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
        DatabaseEngine::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => {
            output.push(0x01);
            write_field(output, 0x01, |field| write_string(field, path));
            write_field(output, 0x02, |field| write_u32(field, *delimiter as u32));
            write_field(output, 0x03, |field| field.push(u8::from(*has_header)));
            write_field(output, 0x04, |field| {
                write_option_u64(field, *infer_schema_length)
            });
        }
        DatabaseEngine::Sql {
            engine,
            connection_string,
            table,
        } => {
            output.push(0x02);
            write_field(output, 0x01, |field| encode_sql_engine(field, engine));
            write_field(output, 0x02, |field| write_string(field, connection_string));
            write_field(output, 0x03, |field| write_string(field, table));
        }
        DatabaseEngine::Parquet { path, columns } => {
            output.push(0x03);
            write_field(output, 0x01, |field| write_string(field, path));
            write_field(output, 0x02, |field| {
                write_option_strings(field, columns.as_deref())
            });
        }
        DatabaseEngine::Excel { path, sheet } => {
            output.push(0x04);
            write_field(output, 0x01, |field| write_string(field, path));
            write_field(output, 0x02, |field| write_string(field, sheet));
        }
        DatabaseEngine::DuckDb { path, table } => {
            output.push(0x05);
            write_field(output, 0x01, |field| write_string(field, path));
            write_field(output, 0x02, |field| write_string(field, table));
        }
        DatabaseEngine::InMemory { name } => {
            output.push(0x06);
            write_field(output, 0x01, |field| write_string(field, name));
        }
    }
}

fn encode_sql_engine(output: &mut Vec<u8>, engine: &DatabaseEngineSql) {
    match engine {
        DatabaseEngineSql::Sqlite { auto_create } => {
            output.push(0x01);
            write_field(output, 0x01, |field| field.push(u8::from(*auto_create)));
        }
        DatabaseEngineSql::Postgres { ssl } => {
            output.push(0x02);
            write_field(output, 0x01, |field| field.push(u8::from(*ssl)));
        }
        DatabaseEngineSql::Mysql { charset } => {
            output.push(0x03);
            write_field(output, 0x01, |field| write_string(field, charset));
        }
    }
}

fn write_field(output: &mut Vec<u8>, tag: u8, encode: impl FnOnce(&mut Vec<u8>)) {
    let mut value = Vec::new();
    encode(&mut value);
    output.push(tag);
    write_u64(output, value.len() as u64);
    output.extend_from_slice(&value);
}

fn write_string(output: &mut Vec<u8>, value: &str) {
    write_bytes(output, value.as_bytes());
}

fn write_bytes(output: &mut Vec<u8>, value: &[u8]) {
    write_u64(output, value.len() as u64);
    output.extend_from_slice(value);
}

fn write_option_u64(output: &mut Vec<u8>, value: Option<usize>) {
    match value {
        Some(value) => {
            output.push(1);
            write_u64(output, value as u64);
        }
        None => output.push(0),
    }
}

fn write_option_strings(output: &mut Vec<u8>, values: Option<&[String]>) {
    match values {
        Some(values) => {
            output.push(1);
            write_u64(output, values.len() as u64);
            for value in values {
                write_string(output, value);
            }
        }
        None => output.push(0),
    }
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
    fn declaration_fingerprint_preserves_the_version_one_digest() {
        let declaration = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };

        assert_eq!(
            fingerprint_declaration(&declaration),
            [
                0x4d, 0x1c, 0x0e, 0xd8, 0x25, 0xa7, 0xbc, 0x7f, 0xfe, 0x6e, 0xb8, 0x57, 0x90, 0xa8,
                0xa3, 0xeb, 0x30, 0x86, 0x50, 0x1f, 0xc2, 0xc2, 0x33, 0x04, 0x67, 0x8e, 0x4f, 0x79,
                0x67, 0x91, 0x05, 0x3f,
            ]
        );
    }
}
