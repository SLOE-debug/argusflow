//! Run Trace 使用的原子文件 I/O 与时间辅助函数。

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

pub(super) fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

pub(super) fn write_json_atomic(
    path: &Path,
    value: &(impl Serialize + ?Sized),
) -> Result<(), String> {
    let temporary_path = path.with_extension("tmp");
    let file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value).map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())?;
    replace_file(path, &temporary_path)
}

pub(super) fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary_path = path.with_extension("tmp");
    let mut file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    replace_file(path, &temporary_path)
}

pub(super) fn read_json<T>(path: &Path) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let file = File::open(path).map_err(|error| error.to_string())?;
    serde_json::from_reader(BufReader::new(file)).map_err(|error| error.to_string())
}

fn replace_file(path: &Path, temporary_path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary_path, path).map_err(|error| error.to_string())
}
