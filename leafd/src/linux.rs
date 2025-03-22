use std::os::unix::net::UnixDatagram;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn notify_systemd(state: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(socket_path) = std::env::var("NOTIFY_SOCKET") {
        if socket_path.is_empty() {
            return Ok(());
        }

        // Удаляем префикс '@' для абстрактных сокетов
        let socket_addr = if socket_path.starts_with('@') {
            format!("\0{}", &socket_path[1..])
        } else {
            socket_path
        };

        // Создаем датаграмный сокет
        let socket = UnixDatagram::unbound()?;

        // Отправляем сообщение в systemd
        socket.send_to(state.as_bytes(), socket_addr)?;

        Ok(())
    } else {
        // Не запущено под systemd
        Ok(())
    }
}

// Уведомляем systemd о готовности
pub fn notify_systemd_ready() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("READY=1")
}

// Уведомляем systemd о завершении
pub fn notify_systemd_stopping() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("STOPPING=1")
}

// Отправляем сигнал watchdog
pub fn notify_systemd_watchdog() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("WATCHDOG=1")
}

// Функция для настройки watchdog таймера

pub async fn setup_watchdog(shutdown: Arc<AtomicBool>) {
    let watchdog_usec = match std::env::var("WATCHDOG_USEC") {
        Ok(value) => match value.parse::<u64>() {
            Ok(usec) => usec,
            Err(_) => return,
        },
        Err(_) => return,
    };

    // Преобразуем микросекунды в миллисекунды и делим на 2 для безопасности
    let interval_ms = watchdog_usec / 1000 / 2;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));

        while !shutdown.load(Ordering::Relaxed) {
            interval.tick().await;
            if let Err(e) = notify_systemd_watchdog() {
                eprintln!("Failed to send watchdog notification: {}", e);
            }
        }
    });
}
