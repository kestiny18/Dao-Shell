use dao_shell::{computer, core::Cancellation};

#[test]
fn local_overview_has_timestamped_partial_observations_without_model_configuration() {
    let snapshot = computer::overview(&Cancellation::default()).unwrap();
    assert!(snapshot["observed_at"].as_str().is_some());
    assert!(snapshot["system"]["disks"].is_array());
    assert!(snapshot["network"]["adapters"].is_array());
    assert!(
        snapshot["network"]["limits"]
            .as_str()
            .unwrap()
            .contains("不主动")
    );
    assert!(snapshot["applications"]["items"].is_array());
    assert!(snapshot["applications"]["limits"].as_str().is_some());
    // Do not print actual host names, process names or application inventory in logs.
}

#[test]
fn cancelled_overview_does_not_begin_sampling() {
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(computer::overview(&cancel).is_err());
}
