use super::*;

#[test]
fn io_write_failure_poisoning_never_advances_watermarks() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    let mut journal = Journal {
        file: File::open(temp.path()).unwrap(),
        written: 7,
        synced: 6,
        failed: false,
    };
    let data = RecordData::Durability { through: 6 };
    assert!(matches!(
        journal.append(data.clone(), 0),
        Err(StorageError::Io(_))
    ));
    assert_eq!(journal.watermarks(), (7, 6));
    assert!(journal.append(data, 1).is_err());
    assert!(journal.sync().is_err());
    assert_eq!(journal.watermarks(), (7, 6));
}
