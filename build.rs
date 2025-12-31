fn main() {
    // Set BUILD_DATE environment variable for compile-time access
    // Use system time to avoid adding chrono as a build dependency
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();

    // Convert Unix timestamp to date (simplified calculation)
    // Days since epoch
    let days = secs / 86400;
    // Calculate year, month, day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days as i64);
    let build_date = format!("{:04}-{:02}-{:02}", year, month, day);

    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // Compile protobuf files for RustDesk protocol
    compile_protos();

    // Rerun if the script itself changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=protos/rendezvous.proto");
    println!("cargo:rerun-if-changed=protos/message.proto");
}

/// Compile protobuf files using prost-build
fn compile_protos() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    prost_build::Config::new()
        .out_dir(&out_dir)
        .compile_protos(
            &["protos/rendezvous.proto", "protos/message.proto"],
            &["protos/"],
        )
        .expect("Failed to compile protobuf files");
}

/// Convert days since Unix epoch to year-month-day
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d)
}
