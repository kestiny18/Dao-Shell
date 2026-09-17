#[cfg(windows)]
#[test]
#[ignore = "Manual Windows CPU check: briefly burns one test-process thread"]
fn busy_test_process_is_visible_in_the_sampling_window() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let worker = std::thread::spawn(move || {
        while stop.load(Ordering::Relaxed) {
            std::hint::black_box(1234567_u64.wrapping_mul(7654321));
        }
    });
    let result = dao_shell::resources::snapshot(
        &dao_shell::resources::Request {
            sample_ms: Some(1500),
            limit: Some(50),
            sort: dao_shell::resources::ProcessSort::Cpu,
            ..Default::default()
        },
        &dao_shell::core::Cancellation::default(),
        true,
    );
    running.store(false, Ordering::Relaxed);
    worker.join().unwrap();
    let result = result.unwrap();
    let row = result["processes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pid"] == std::process::id())
        .expect("busy test process missing from CPU results");
    let cpu = row["cpu_percent_one_core"]
        .as_f64()
        .expect("CPU sample unavailable");
    println!("Test worker CPU (one core): {cpu:.1}%");
    assert!(
        cpu > 5.0,
        "A busy test thread was reported near idle: {cpu}"
    );
}
