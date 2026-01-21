use metrics::{describe_gauge, gauge};
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
use tokio::time::interval;

pub fn start_system_metrics_task() {
    describe_gauge!(
        "fuzzle_process_memory_rss_bytes",
        metrics::Unit::Bytes,
        "Resident Set Size (RSS) memory usage of the process"
    );
    describe_gauge!(
        "fuzzle_process_memory_virtual_bytes",
        metrics::Unit::Bytes,
        "Virtual memory usage of the process"
    );
    describe_gauge!(
        "fuzzle_process_cpu_usage_percent",
        metrics::Unit::Percent,
        "CPU usage percent of the process"
    );
    describe_gauge!(
        "fuzzle_process_uptime_seconds",
        metrics::Unit::Seconds,
        "Uptime of the process in seconds"
    );
    describe_gauge!(
        "fuzzle_process_open_fds",
        metrics::Unit::Count,
        "Number of open file descriptors"
    );

    tokio::spawn(async move {
        let pid = Pid::from_u32(std::process::id());

        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );

        let mut ticker = interval(Duration::from_secs(5));

        loop {
            ticker.tick().await;

            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::everything()
                    .without_disk_usage()
                    .without_tasks(),
            );

            if let Some(process) = sys.process(pid) {
                gauge!("fuzzle_process_memory_rss_bytes").set(process.memory() as f64);
                gauge!("fuzzle_process_memory_virtual_bytes").set(process.virtual_memory() as f64);

                gauge!("fuzzle_process_cpu_usage_percent").set(process.cpu_usage() as f64);
                gauge!("fuzzle_process_uptime_seconds").set(process.run_time() as f64);

                if let Some(files) = process.open_files() {
                    gauge!("fuzzle_process_open_fds").set(files as f64);
                }
            }
        }
    });
}
