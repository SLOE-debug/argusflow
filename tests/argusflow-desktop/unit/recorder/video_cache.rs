use super::super::index::{Entry, Frame, Header, Source};
use super::*;
fn selected(root: &Path) -> Selected {
    Selected {
        directory: root.join("video/000001"),
        header: Header {
            qpc_origin: 0,
            qpc_frequency: 1000,
            width: 2,
            height: 2,
            source: Source {
                origin: [0, 0],
                dpi: [96, 96],
            },
        },
        entry: Entry {
            frame: Frame {
                sequence: 1,
                pts_100ns: 0,
                presented_qpc: 0,
                acquired_qpc: 1,
                repeated: false,
            },
            duration_100ns: 10000,
        },
        at: 0,
    }
}
#[test]
fn disk_cache_survives_reopen_and_missing_images_are_rebuilt() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let selected = selected(root);
    std::fs::create_dir_all(&selected.directory).unwrap();
    std::fs::write(selected.directory.join("screen.mp4"), b"video").unwrap();
    let image = argusflow_recorder::publish_review_image(root, b"derived", [2, 2]).unwrap();
    Cache::open(root, &selected)
        .unwrap()
        .put(root, &image)
        .unwrap();
    let cached = Cache::open(root, &selected)
        .unwrap()
        .get(root)
        .unwrap()
        .unwrap();
    assert_eq!(cached.1, b"derived");
    std::fs::remove_file(root.join(&image.path)).unwrap();
    assert!(
        Cache::open(root, &selected)
            .unwrap()
            .get(root)
            .unwrap()
            .is_none()
    );
}
#[test]
fn changed_video_cannot_reuse_previous_pixels() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let selected = selected(root);
    std::fs::create_dir_all(&selected.directory).unwrap();
    std::fs::write(selected.directory.join("screen.mp4"), b"video").unwrap();
    let image = argusflow_recorder::publish_review_image(root, b"derived", [2, 2]).unwrap();
    Cache::open(root, &selected)
        .unwrap()
        .put(root, &image)
        .unwrap();
    std::fs::write(selected.directory.join("screen.mp4"), b"changed video").unwrap();
    assert!(
        Cache::open(root, &selected)
            .unwrap()
            .get(root)
            .unwrap()
            .is_none()
    );
}
