use arboard::Clipboard;
use log::{info, warn};

pub fn set_text(text: String) {
    info!("Set clip text to {}", text);
    match Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(text.clone()) {
                warn!("Failed to get clipboard text {}", e);
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
                    info!("Got clip text to {}", text);
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