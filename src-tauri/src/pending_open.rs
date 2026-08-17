use std::sync::Mutex;

#[derive(Default)]
pub struct PendingOpen {
    latest: Mutex<Option<String>>,
}

impl PendingOpen {
    pub fn put(&self, path: String) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(path);
        }
    }

    pub fn take(&self) -> Option<String> {
        self.latest.lock().ok()?.take()
    }
}

#[tauri::command]
pub fn take_pending_open_file(state: tauri::State<'_, PendingOpen>) -> Option<String> {
    state.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_request_wins() {
        let state = PendingOpen::default();
        state.put("a.mp4".into());
        state.put("b.mp4".into());
        assert_eq!(state.take().as_deref(), Some("b.mp4"));
        assert_eq!(state.take(), None);
    }
}
