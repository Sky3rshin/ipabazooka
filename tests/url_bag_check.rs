//! Network check (run by hand): makes the app's very first sign-in request,
//! Apple's "URL bag", with the same HTTP client the sign-in uses, and prints
//! how long it took and every underlying reason if it fails.
//!
//!     cargo test --test url_bag_check -- --ignored --nocapture

const URL_BAG: &str = "https://gsa.apple.com/grandslam/GsService2/lookup";

#[tokio::test]
#[ignore]
async fn url_bag_can_be_fetched() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = isideload::auth::grandslam::GrandSlam::build_reqwest_client(false, None).expect("client");
    for attempt in 1..=3 {
        let t = std::time::Instant::now();
        match client.get(URL_BAG).send().await {
            Ok(r) => {
                let status = r.status();
                let len = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                println!("try {attempt}: HTTP {status}, {len} bytes, {:?}", t.elapsed());
            }
            Err(e) => {
                println!("try {attempt}: failed after {:?}: {e}", t.elapsed());
                let mut src = std::error::Error::source(&e);
                while let Some(s) = src {
                    println!("    caused by: {s}");
                    src = s.source();
                }
            }
        }
    }
}
