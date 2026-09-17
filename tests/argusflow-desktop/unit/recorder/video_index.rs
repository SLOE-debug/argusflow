use super::*;
fn select(root: &Path, target: i64, direction: i8) -> Result<Selected, String> {
    super::select(
        root,
        target,
        direction,
        &mut super::super::index_cache::IndexCache::default(),
        None,
        None,
    )
}
fn segment(root: &Path, name: &str, origin: i64) {
    let path = root.join("video").join(name);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("session.json"),serde_json::json!({"qpc_origin":origin,"qpc_frequency":10_000_000,"width":640,"height":360,"source":{"origin":[-640,0],"dpi":[96,96]}}).to_string()).unwrap();
    let mut text = String::new();
    for (seq, pts) in [0, 10_000_000, 20_000_000].into_iter().enumerate() {
        text.push_str(&serde_json::json!({"frame":{"sequence":seq+1,"pts_100ns":pts,"presented_qpc":origin+pts,"acquired_qpc":origin+pts+100,"repeated":false},"duration_100ns":10_000_000}).to_string());
        text.push('\n');
    }
    std::fs::write(path.join("frames.jsonl"), text).unwrap();
}
#[test]
fn seeks_neighbors_without_crossing_pause_gap() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    segment(root, "000001", 100);
    segment(root, "000002", 100_000_100);
    assert_eq!(select(root, 15_000_100, 0).unwrap().entry.frame.sequence, 2);
    assert_eq!(
        select(root, 10_000_100, -1).unwrap().entry.frame.sequence,
        1
    );
    assert_eq!(select(root, 10_000_100, 1).unwrap().entry.frame.sequence, 3);
    assert!(select(root, 50_000_100, 0).is_err());
    assert_eq!(
        select(root, 100_000_100, 0).unwrap().entry.frame.sequence,
        1
    );
}
#[test]
fn partial_index_tail_does_not_fabricate_frame() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    segment(temp.path(), "000001", 100);
    let path = temp.path().join("video/000001/frames.jsonl");
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(b"{\"frame\":")
        .unwrap();
    assert_eq!(
        select(temp.path(), 25_000_100, 0)
            .unwrap()
            .entry
            .frame
            .sequence,
        3
    );
    assert!(select(temp.path(), 35_000_100, 0).is_err());
}

#[test]
fn cached_index_refreshes_when_recording_grows() {
    let temp = tempfile::tempdir().unwrap();
    segment(temp.path(), "000001", 100);
    let mut cache = super::super::index_cache::IndexCache::default();
    assert_eq!(
        super::select(temp.path(), 25_000_100, 0, &mut cache, None, None)
            .unwrap()
            .entry
            .frame
            .sequence,
        3
    );
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(temp.path().join("video/000001/frames.jsonl"))
        .unwrap();
    writeln!(file, "{}", serde_json::json!({"frame":{"sequence":4,"pts_100ns":30_000_000,"presented_qpc":30_000_100,"acquired_qpc":30_000_101,"repeated":false},"duration_100ns":10_000_000})).unwrap();
    assert_eq!(
        super::select(temp.path(), 35_000_100, 0, &mut cache, None, None)
            .unwrap()
            .entry
            .frame
            .sequence,
        4
    );
}

#[test]
fn thirty_minute_index_uses_cached_binary_lookup() {
    let temp = tempfile::tempdir().unwrap();
    segment(temp.path(), "000001", 100);
    use std::io::Write;
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(temp.path().join("video/000001/frames.jsonl")).unwrap(),
    );
    for frame in 0..54000i64 {
        writeln!(file, "{}", serde_json::json!({"frame":{"sequence":frame+1,"pts_100ns":frame*333333,"presented_qpc":100+frame*333333,"acquired_qpc":101+frame*333333,"repeated":false},"duration_100ns":333333})).unwrap();
    }
    drop(file);
    let mut cache = super::super::index_cache::IndexCache::default();
    let started = std::time::Instant::now();
    assert_eq!(
        super::select(temp.path(), 53999 * 333333 + 100, 0, &mut cache, None, None)
            .unwrap()
            .entry
            .frame
            .sequence,
        54000
    );
    println!("30MIN_INDEX_LOAD={:?}", started.elapsed());
    let started = std::time::Instant::now();
    for frame in [1, 53000, 200, 54000] {
        assert_eq!(
            super::select(
                temp.path(),
                (frame - 1) * 333333 + 100,
                0,
                &mut cache,
                None,
                None
            )
            .unwrap()
            .entry
            .frame
            .sequence,
            frame as u64
        );
    }
    println!("FOUR_CACHED_LOOKUPS={:?}", started.elapsed());
}

#[test]
fn result_window_stops_at_segment_end_without_crossing_pause() {
    let temp = tempfile::tempdir().unwrap();
    segment(temp.path(), "000001", 100);
    segment(temp.path(), "000002", 100_000_100);
    let mut cache = super::super::index_cache::IndexCache::default();
    let frame = super::select(
        temp.path(),
        110_000_100,
        0,
        &mut cache,
        Some(25_000_100),
        None,
    )
    .unwrap();
    assert_eq!(frame.directory.file_name().unwrap(), "000001");
    assert_eq!(frame.entry.frame.sequence, 3);
    assert!(
        super::select(
            temp.path(),
            110_000_100,
            0,
            &mut cache,
            Some(50_000_100),
            None
        )
        .is_err()
    );
}
