use super::declaration::DatabaseDecl;
use super::engine::{DatabaseEngine, DatabaseEngineSql};

const FINGERPRINT_VERSION: &[u8] = b"yssbi.database-declaration.fingerprint.v1";

pub(super) fn fingerprint_declaration(declaration: &DatabaseDecl) -> Vec<u8> {
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
    sha256(&encoding).to_vec()
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

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66b, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let mut working = state;
        for index in 0..64 {
            let s1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let choice = (working[4] & working[5]) ^ (!working[4] & working[6]);
            let temp1 = working[7]
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let s0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let temp2 = s0.wrapping_add(majority);
            working = [
                temp1.wrapping_add(temp2),
                working[0],
                working[1],
                working[2],
                working[3].wrapping_add(temp1),
                working[4],
                working[5],
                working[6],
            ];
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(working[index]);
        }
    }

    let mut output = [0; 32];
    for (index, word) in state.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}
