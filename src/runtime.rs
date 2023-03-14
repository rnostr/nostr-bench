use tokio::runtime::{Builder, Runtime};

/// Creates a tokio runtime with n amount of workers as specified by the
/// parameters.
pub fn get_rt(workers: usize) -> Runtime {
    let mut rt = Builder::new_multi_thread();
    rt.enable_all();
    if workers > 0 {
        rt.worker_threads(workers);
    }
    rt.build().expect("Failed to build the runtime.")
}
