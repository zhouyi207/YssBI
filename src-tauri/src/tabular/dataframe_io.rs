use std::fs::{self, File};
use std::path::Path;

use polars::prelude::{
    CsvWriter, DataFrame, IpcReader, IpcWriter, ParquetWriter, SerReader, SerWriter,
};

pub fn read_ipc_dataframe(path: &Path) -> Result<DataFrame, String> {
    let file =
        File::open(path).map_err(|error| format!("Failed to open {}: {error}", path.display()))?;
    IpcReader::new(file)
        .finish()
        .map_err(|error| format!("Failed to read Arrow IPC {}: {error}", path.display()))
}

pub fn write_ipc_dataframe(path: &Path, dataframe: &mut DataFrame) -> Result<(), String> {
    create_parent_directory(path)?;
    let mut file = File::create(path)
        .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    IpcWriter::new(&mut file)
        .finish(dataframe)
        .map_err(|error| format!("Failed to write Arrow IPC {}: {error}", path.display()))
}

pub fn write_csv_dataframe(path: &Path, dataframe: &mut DataFrame) -> Result<(), String> {
    create_parent_directory(path)?;
    let file = File::create(path)
        .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    CsvWriter::new(file)
        .finish(dataframe)
        .map_err(|error| format!("Failed to write CSV {}: {error}", path.display()))
}

pub fn write_parquet_dataframe(path: &Path, dataframe: &mut DataFrame) -> Result<(), String> {
    create_parent_directory(path)?;
    let file = File::create(path)
        .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    ParquetWriter::new(file)
        .finish(dataframe)
        .map(|_| ())
        .map_err(|error| format!("Failed to write Parquet {}: {error}", path.display()))
}

fn create_parent_directory(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use polars::prelude::{Column, DataFrame};

    use super::{read_ipc_dataframe, write_csv_dataframe, write_ipc_dataframe};

    #[test]
    fn dataframe_round_trips_through_ipc_and_exports_csv() {
        let directory =
            std::env::temp_dir().join(format!("yssbi-dataframe-io-{}", uuid::Uuid::new_v4()));
        let ipc_path = directory.join("nested/data.arrow");
        let csv_path = directory.join("nested/data.csv");
        let mut dataframe = DataFrame::new(
            2,
            vec![
                Column::new("name".into(), ["a", "b"]),
                Column::new("value".into(), [1_i64, 2]),
            ],
        )
        .expect("dataframe");

        write_ipc_dataframe(&ipc_path, &mut dataframe).expect("write IPC");
        let mut restored = read_ipc_dataframe(&ipc_path).expect("read IPC");
        write_csv_dataframe(&csv_path, &mut restored).expect("write CSV");
        let csv = std::fs::read_to_string(csv_path).expect("read CSV");
        std::fs::remove_dir_all(directory).expect("remove test directory");

        assert_eq!(restored.height(), 2);
        assert_eq!(csv, "name,value\na,1\nb,2\n");
    }
}
