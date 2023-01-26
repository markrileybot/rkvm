use arboard::{Clipboard, Error};
use log::warn;

pub fn set_text(text: String) {
    match Clipboard::new() {
        Ok(mut clipboard) => {
            match clipboard.set_text(text.clone()) {
                Ok(_) => {
                    warn!("Set clip text to {}", text);
                }
                Err(e) => {
                    warn!("Failed to get clipboard text {}", e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to get clipboard {}", e);
        }
    }
}

pub fn get_text() -> Option<String> {
    match Clipboard::new() {
        Ok(mut clipboard) => {
            match clipboard.get_text() {
                Ok(text) => {
                    warn!("Got clip text to {}", text);
                    return Some(text);
                }
                Err(e) => {
                    warn!("Failed to get clipboard text {}", e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to get clipboard {}", e);
        }
    }
    None
}