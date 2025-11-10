use std::sync::Arc;

use reqwest::cookie::Jar;

struct QuerySesion {
    cookies_jar: Arc<Jar>,
}

impl QuerySesion {
    pub fn new() -> Self {
        Self {
            cookies_jar: Arc::new(Jar::default()),
        }
    }

    pub fn from_jar(jar: Arc<Jar>) -> Self {
        Self { cookies_jar: jar }
    }
}
