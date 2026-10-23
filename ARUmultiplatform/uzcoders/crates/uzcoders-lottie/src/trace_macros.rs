macro_rules! trace {
    ($($tt:tt)*) => {{}};
}

macro_rules! span_enter {
    ($level:expr, $name:literal $(, $($fields:tt)+)? ) => {
        let _span = ();
    };
}
