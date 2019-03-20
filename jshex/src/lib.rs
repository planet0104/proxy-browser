#[macro_use]
extern crate stdweb;

use stdweb::js_export;

#[js_export]
fn encode(text: String) -> String {
    hex::encode(text)
}