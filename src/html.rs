use std::fmt::Display;

use build_html::Html;

#[derive(Debug)]
pub struct Bold {
    content: String,
}

impl Html for Bold {
    fn to_html_string(&self) -> String {
        format!("<b>{}</b>", &self.content)
    }
}

impl<T: Display> From<T> for Bold {
    fn from(value: T) -> Self {
        Bold {
            content: value.to_string(),
        }
    }
}
