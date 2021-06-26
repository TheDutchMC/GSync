#[macro_export]
macro_rules! unwrap_str {
    ($expression:expr) => {
        match $expression {
            Ok(t) => t,
            Err(e) => return Err(e.to_string())
        }
    }
}

/// This macro is used for dealing with responses from the Google API
///
/// The struct passed in as the first argument should contain the field `error: Option<GoogleError`.
/// The second argument is the field in the struct that should be returned when there is no error
///
/// ## Example:
/// ```
/// use crate::api::GoogleError
///
/// struct Foo {
///     error:  Option<GoogleError>
///     bar:    Option<String>          // We make this an Option, because when Google returns an error, it won't give the data we asked it for
/// }
///
/// fn baz() -> Return<String, String> {
///     let foo = Foo { error: None, bar: String::new() };
///     let bar = google_error!(foo, bar);
///     Ok(bar)
/// }
/// ```
///
/// This would expand to:
/// ```
/// use crate::api::GoogleError
///
/// struct Foo {
///     error:  Option<GoogleError>
///     bar:    Option<String>
/// }
///
/// fn baz() -> Return<String, String> {
///     let foo = Foo { error: None, bar: String::new() };
///     let bar = if foo.error.is_some() {
///         return Err(format!("{:?}", foo.error));
///     } else {
///         foo.bar.unwrap()
///     }
///
///     Ok(bar)
/// }
#[macro_export]
macro_rules! google_error {
    ($expression:expr, $ident:ident) => {
        if $expression.error.is_some() {
            return Err(format!("{:?}", $expression.error));
        } else {
            $expression.$ident.unwrap()
        }
    }
}