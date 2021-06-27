/// Macro for handling errors returned from the `rusqlite` crate
///
/// The argument of this macro invoication should be a `Result<T, rusqlite::Error>`
#[macro_export]
macro_rules! unwrap_db_err {
    ($expression:expr) => {
        match $expression {
            Ok(t) => t,
            Err(e) => return Err($crate::Error::DatabaseError(e))
        }
    }
}

/// Macro for handling errors returned from the `reqwest` crate
///
/// The argument of this macro_invocation should be a `Result<T, reqwest::Error>`
#[macro_export]
macro_rules! unwrap_req_err {
    ($expression:expr) => {
        match $expression {
            Ok(t) => t,
            Err(e) => return Err($crate::Error::RequestError(e))
        }
    }
}

/// Macro for handling errors that fit into no category
///
/// The argument of this macro invocation should be a `Result<T, P: ToString>`
#[macro_export]
macro_rules! unwrap_other_err {
    ($expression:expr) => {
        match $expression {
            Ok(t) => t,
            Err(e) => return Err($crate::Error::Other(e.to_string()))
        }
    }
}

/// Handle a Result<T, crate::Error>
///
/// When the passed in Result is `Ok`, this macro will return `T`.
/// When the passed in Result is `Err`, this macro will print out the Error in a nice way to stderr and exit with exit code 1
///
#[macro_export]
macro_rules! handle_err {
    ($expression:expr) => {
        match $expression {
            Ok(t) => t,
            Err(e) => {
                match e {
                    $crate::Error::DatabaseError(e) => eprintln!("Error: An error occurred while processing or handling database data: {:?}", e),
                    $crate::Error::RequestError(e) => eprintln!("Error: An error occurred while sending a HTTP request: {:?}", e),
                    $crate::Error::GoogleError(e) => eprintln!("Error: The Google API returned an error: {:?}", e),
                    $crate::Error::Other(e) => eprintln!("Error: An error occurred: {:?}", e)
                }

                eprintln!("This is a fatal error. Exiting!");
                std::process::exit(1);
            }
        }
    }
}

/// This macro is used for dealing with responses from the Google API
///
/// The struct passed in as the first argument should be of type GoogleResponse<T>
///
/// ## Example:
/// ```
/// use crate::api::GoogleError
/// use crate::api::GoogleResponse
///
/// struct Foo {
///     bar:    String
/// }
///
/// fn baz() -> Return<String, String> {
///     let response: GoogleResponse<Foo> = some_request();
///
///     // `foo` is of type Foo
///     let foo = google_error!(response)
///     Ok(bar)
/// }
/// ```
///
/// This would expand to:
/// ```
/// use crate::api::GoogleError
/// use crate::api::GoogleResponse
///
/// struct Foo {
///     bar:    String
/// }
///
/// fn baz() -> Return<String, String> {
///     let response: GoogleResponse<Foo> = some_request();
///
///     // `foo` is of type Foo
///     let foo = if response.error.is_some() {
///         return Err(format!("{:?}", foo.error));
///     } else {
///         response.data.unwrap()
///     }
///
///     Ok(foo.bar)
/// }
#[macro_export]
macro_rules! unwrap_google_err {
    ($expression:expr) => {
        if $expression.error.is_some() {
            return Err($crate::Error::GoogleError($expression.error.unwrap()));
        } else {
            $expression.data.unwrap()
        }
    }
}