use stderrlog::{self, LogLevelNum};

pub fn init_logger() {
  stderrlog::new()
    .verbosity(LogLevelNum::Info)
    // .timestamp(stderrlog::Timestamp::Off)
    .init()
    .unwrap();
}
