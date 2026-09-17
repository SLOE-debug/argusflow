use super::*;
#[test]
fn derived_cache_evicts_only_old_derived_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("attachments")).unwrap();
    std::fs::write(root.join("attachments/original.png"), b"evidence").unwrap();
    let first = publish_review_image(root, b"first", [1, 1]).unwrap();
    std::fs::File::options()
        .write(true)
        .open(root.join(&first.path))
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    for i in 0u32..32 {
        publish_review_image(root, &i.to_le_bytes(), [1, 1]).unwrap();
    }
    assert!(!root.join(first.path).exists());
    assert_eq!(
        std::fs::read_dir(root.join("review-cache"))
            .unwrap()
            .count(),
        32
    );
    assert_eq!(
        std::fs::read(root.join("attachments/original.png")).unwrap(),
        b"evidence"
    );
}
#[test]
fn unknown_cache_content_is_not_deleted() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("review-cache")).unwrap();
    let path = root.join("review-cache/note.txt");
    std::fs::write(&path, b"keep").unwrap();
    assert!(publish_review_image(root, b"image", [1, 1]).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"keep");
}
