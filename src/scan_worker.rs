use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;

use crate::hid_scan;
use crate::UserEvent;

const DEBOUNCE: Duration = Duration::from_millis(400);

enum ScanRequest {
    Scan,
    Shutdown,
}

/// 后台 HID 扫描：防抖 + 请求合并，结果通过 `DevicesUpdated` 回主线程。
pub struct ScanWorker {
    tx: Sender<ScanRequest>,
}

impl ScanWorker {
    pub fn spawn(proxy: EventLoopProxy<UserEvent>) -> Self {
        let (tx, rx) = mpsc::channel::<ScanRequest>();
        thread::spawn(move || worker_loop(proxy, rx));
        Self { tx }
    }

    pub fn request_scan(&self) {
        let _ = self.tx.send(ScanRequest::Scan);
    }
}

impl Drop for ScanWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(ScanRequest::Shutdown);
    }
}

fn worker_loop(proxy: EventLoopProxy<UserEvent>, rx: Receiver<ScanRequest>) {
    let mut pending = false;
    let mut deadline = Instant::now();

    loop {
        if !pending {
            match rx.recv() {
                Ok(ScanRequest::Shutdown) | Err(_) => break,
                Ok(ScanRequest::Scan) => {
                    pending = true;
                    deadline = Instant::now() + DEBOUNCE;
                }
            }
            continue;
        }

        let wait = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(ScanRequest::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(ScanRequest::Scan) => {
                // 合并请求，重新计时
                deadline = Instant::now() + DEBOUNCE;
            }
            Err(RecvTimeoutError::Timeout) => {
                pending = false;
                let devices = hid_scan::scan_mchose_devices().unwrap_or_default();
                if proxy
                    .send_event(UserEvent::DevicesUpdated(devices))
                    .is_err()
                {
                    break;
                }

                // 扫描期间堆积的请求：再防抖一轮
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ScanRequest::Shutdown => return,
                        ScanRequest::Scan => {
                            pending = true;
                            deadline = Instant::now() + DEBOUNCE;
                        }
                    }
                }
            }
        }
    }
}
