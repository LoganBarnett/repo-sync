use stderrlog;

pub fn init_logger() {
  stderrlog::new()
    .module(module_path!())
    .verbosity(4) // Set verbosity to INFO level
    // .timestamp(stderrlog::Timestamp::Off)
    .init()
    .unwrap();
}
