#[macro_export]
/// A debug statement that prints a message when invoked. Only available on the `debug`
/// crate feature.
macro_rules! println {
    () => {
        #[cfg(feature = "debug")]
        {
            defmt::println!();
        }
    };
    ($($arg:tt)*) => {{
        #[cfg(feature = "debug")]
        {
            defmt::println!($($arg)*);
        }
    }}
}
