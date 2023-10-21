use std::str::FromStr;

use askama::Template;
use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};

pub struct HtmlTemplate<T> {
    pub template: T,
    pub headers: HeaderMap,
}

impl<T: Template> HtmlTemplate<T> {
    pub fn new(template: T) -> Self {
        HtmlTemplate {
            template,
            headers: HeaderMap::new(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        let header_name = HeaderName::from_str(name).expect("Invalid header name");
        let header_value = HeaderValue::from_str(value).expect("Invalid header value");
        self.headers.insert(header_name, header_value);
        self
    }
}

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.template.render() {
            Ok(html) => {
                let mut response = Html(html).into_response();
                for (name, value) in self.headers {
                    response.headers_mut().insert(name.unwrap(), value);
                }
                response
            }
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}
