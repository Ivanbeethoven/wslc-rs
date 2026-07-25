#![cfg(feature = "integration")]
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wslc::{com, raw, Service, Session};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

#[test]
fn init_settings_then_version() {
    // Step 1: Version works
    let v1 = Service::version().expect("version before init_settings");
    println!("v1 OK: {v1:?}");

    // Step 2: Init session settings (same thing start() does before create_session)
    let _com = com::try_initialize_mta().expect("COM init");
    let sdk = raw::sdk().expect("sdk");
    let name = unique_name("test");
    let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let storage = r"C:\WslcData\test-storage";
    let storage_wide: Vec<u16> = storage.encode_utf16().chain(std::iter::once(0)).collect();
    let mut settings = wslc_sys::WslcSessionSettings::default();
    let hr = sdk.init_session_settings(name_wide.as_ptr(), storage_wide.as_ptr(), &mut settings);
    println!("init_session_settings hr=0x{hr:08X}");

    // Step 3: Version still works AFTER init_session_settings
    let v2 = Service::version().expect("version after init_settings");
    println!("v2 OK: {v2:?}");

    // Step 4: CreateSession directly
    let raw_session = raw::map_result(sdk.create_session(&mut settings));
    match raw_session {
        Ok(s) => println!("create_session OK: {s:?}"),
        Err(e) => println!("create_session FAILED: {e:?}"),
    }
}
