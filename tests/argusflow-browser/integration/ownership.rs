//! 在自建真实浏览器上检验外部连接语义及进程/配置目录回收。
use argusflow_browser::{Browser, BrowserConfig, LaunchOptions, OperationOptions};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
fn profiles() -> HashSet<PathBuf> {
    std::fs::read_dir(std::env::temp_dir().join("argusflow-browser"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}
fn launch() -> LaunchOptions {
    let mut options = LaunchOptions::new(
        std::env::var_os("ARGUSFLOW_TEST_BROWSER").expect("set ARGUSFLOW_TEST_BROWSER"),
    );
    options.headless = true;
    options
}
fn options() -> OperationOptions {
    OperationOptions::default()
}
async fn removed(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while path.exists() {
        assert!(
            Instant::now() < deadline,
            "profile still exists: {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

#[tokio::test]
#[ignore = "launches a real isolated browser; do not run concurrent ownership tests"]
async fn external_connections_detach_and_owned_browser_drop_cleans_profile() {
    let before = profiles();
    let owner = Browser::launch(launch(), BrowserConfig::default())
        .await
        .unwrap();
    let created = profiles().difference(&before).cloned().collect::<Vec<_>>();
    assert_eq!(created.len(), 1);
    let profile = &created[0];
    let endpoint = std::fs::read_to_string(profile.join("DevToolsActivePort")).unwrap();
    let mut lines = endpoint.lines();
    let port = lines.next().unwrap();
    let ws_path = lines.next().unwrap();
    let page = owner.new_page("about:blank", options()).await.unwrap();
    let target = page.target_id().to_owned();
    let http = Browser::connect(
        &format!("http://127.0.0.1:{port}"),
        BrowserConfig::default(),
        options(),
    )
    .await
    .unwrap();
    assert!(
        http.pages(options())
            .await
            .unwrap()
            .iter()
            .any(|page| page.target_id() == target)
    );
    let external_page = http.attach(&target, options()).await.unwrap();
    assert_eq!(external_page.evaluate("1+2", options()).await.unwrap(), 3);
    http.shutdown(options()).await.unwrap();
    assert_eq!(page.evaluate("2+3", options()).await.unwrap(), 5);
    let ws = Browser::connect(
        &format!("ws://127.0.0.1:{port}{ws_path}"),
        BrowserConfig::default(),
        options(),
    )
    .await
    .unwrap();
    let attached = ws.attach(&target, options()).await.unwrap();
    attached.detach(options()).await.unwrap();
    ws.shutdown(options()).await.unwrap();
    assert_eq!(page.evaluate("3+4", options()).await.unwrap(), 7);
    println!(
        "PASS explicit HTTP/WS endpoints, attach/detach and external shutdown preserves browser/page"
    );
    drop(owner);
    removed(profile).await;
    assert!(!page.is_open());
    println!("PASS owned browser Drop closes connection and removes temporary profile");

    let before = profiles();
    let owner = Browser::launch(launch(), BrowserConfig::default())
        .await
        .unwrap();
    let created = profiles().difference(&before).cloned().collect::<Vec<_>>();
    assert_eq!(created.len(), 1);
    owner.shutdown(options()).await.unwrap();
    assert!(!created[0].exists());
    println!("PASS explicit shutdown waits for process/profile cleanup");

    let before = profiles();
    let mut short = launch();
    short.timeout = Duration::from_millis(1);
    assert!(
        Browser::launch(short, BrowserConfig::default())
            .await
            .is_err()
    );
    for path in profiles().difference(&before) {
        removed(path).await;
    }
    println!("PASS startup timeout cleans isolated profile");
}
