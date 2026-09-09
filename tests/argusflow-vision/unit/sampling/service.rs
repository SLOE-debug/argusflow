use super::*;
struct StalledSource;
impl RegionSource for StalledSource {
    fn sample(&self, _: SampleRequest, _: Operation) -> CaptureFuture<RegionSample> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn stalled_source_obeys_outer_total_deadline() {
    let result = run(
        Arc::new(StalledSource),
        Key {
            source: SourceId(1),
            region: PixelRect::new(0, 0, 4, 4).unwrap(),
        },
        None,
        Operation::new(OperationOptions::new(Duration::from_millis(20)).unwrap()),
        |_, _| async { panic!("inference must not start") },
    )
    .await;
    assert!(
        matches!(result,Err(SampledOcrError::Capture(error)) if error.kind()==FailureKind::Timeout)
    );
}
#[path = "../../support/region_source.rs"]
mod region_source;
#[test]
fn cache_does_not_evict_inflight_work() {
    let key = Key {
        source: SourceId(1),
        region: PixelRect::new(0, 0, 8, 8).unwrap(),
    };
    let other = Key {
        source: SourceId(2),
        ..key
    };
    let (result, _) = watch::channel(None);
    let flight = Arc::new(Flight {
        operation: Operation::new(OperationOptions::default()),
        users: AtomicUsize::new(2),
        result,
    });
    let mut cache = Cache {
        entries: HashMap::new(),
        tick: 0,
        capacity: 1,
    };
    cache.entries.insert(
        key,
        Entry {
            cached: None,
            flight: Some(flight.clone()),
            used: 0,
        },
    );
    assert!(!cache.reserve(other));
    drop(Waiter(flight.clone()));
    assert!(!flight.operation.is_cancelled());
    drop(Waiter(flight.clone()));
    assert!(flight.operation.is_cancelled());
    cache.entries.get_mut(&key).unwrap().flight = None;
    assert!(cache.reserve(other));
    assert!(cache.entries.is_empty());
}

#[tokio::test]
async fn identical_content_reuses_inference_and_tracks_new_revision() {
    let budget = ByteBudget::new(64).unwrap();
    let image = PixelImage::new(
        4,
        4,
        16,
        argusflow_capture_contracts::PixelFormat::Bgrx8,
        vec![255; 64],
        budget.reserve(64).unwrap(),
    )
    .unwrap();
    let source = region_source::FixtureSource::new(image);
    let key = Key {
        source: SourceId(1),
        region: PixelRect::new(0, 0, 4, 4).unwrap(),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let first = run(
        source.clone(),
        key,
        None,
        Operation::new(OperationOptions::default()),
        move |input, _| async move {
            assert!(matches!(input, ImageInput::Shared(_)));
            counted.fetch_add(1, Ordering::Relaxed);
            Ok(OcrResult {
                blocks: Vec::new(),
                width: 4,
                height: 4,
            })
        },
    )
    .await
    .unwrap();
    source.revision.store(2, Ordering::Release);
    let counted = calls.clone();
    let second = run(
        source,
        key,
        Some(first),
        Operation::new(OperationOptions::default()),
        move |_, _| async move {
            counted.fetch_add(1, Ordering::Relaxed);
            Ok(OcrResult {
                blocks: Vec::new(),
                width: 4,
                height: 4,
            })
        },
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(second.output.reused());
    assert_eq!(second.output.version().revision, 2);
}

#[tokio::test]
async fn revoked_during_inference_never_populates_cache() {
    let budget = ByteBudget::new(64).unwrap();
    let image = PixelImage::new(
        4,
        4,
        16,
        argusflow_capture_contracts::PixelFormat::Bgrx8,
        vec![255; 64],
        budget.reserve(64).unwrap(),
    )
    .unwrap();
    let source = region_source::FixtureSource::new(image);
    let valid = source.valid.clone();
    let key = Key {
        source: SourceId(1),
        region: PixelRect::new(0, 0, 4, 4).unwrap(),
    };
    let result = run(
        source,
        key,
        None,
        Operation::new(OperationOptions::default()),
        move |_, _| async move {
            valid.store(false, Ordering::Release);
            Ok(OcrResult {
                blocks: Vec::new(),
                width: 4,
                height: 4,
            })
        },
    )
    .await;
    assert!(
        matches!(result,Err(SampledOcrError::Capture(error)) if error.kind()==FailureKind::StaleHandle)
    );
}
