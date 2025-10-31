use captains_log::{recipe, ConsoleTarget, Level};

pub mod net;
pub mod runtime;
pub mod time;

// Initialize logging in the test utility crate
pub fn init_logger() {
    recipe::console_logger(ConsoleTarget::Stdout, Level::Debug)
        .test()
        .build()
        .expect("Failed to initialize logger");
}
