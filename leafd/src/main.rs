mod server;
mod socket; // Объявление внутреннего модуля сокета
mod stor; // Объявление внутреннего модуля хранилища

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use server::Server;

#[cfg(target_os = "linux")]
use linux::*;

#[cfg(target_os = "windows")]
use windows::*;

use consts::*; // Внутренний модуль с константами

#[cfg(target_os = "linux")]
mod consts {
    pub const APPS_DIR_ABS_PATH: &str = "/var/local"; // Абсолютный путь к корню директории хранилища
    pub const APP_DIR: &str = "leaf"; // Директория приложения
    pub const CHUNKS_DIR: &str = "chunks"; // Директория чанков
}

#[tokio::main]
#[cfg(target_os = "linux")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering}; // Для флага завершения
    use std::sync::Arc; // Для разделяемого владения // Зависимость стандартной библиотеки для работы с файловыми путями

    use tokio::select; // Для одновременного ожидания нескольких событий
    use tokio::signal::unix::{signal, SignalKind};
    use tokio::sync::mpsc::channel; // Внешняя зависимость для использования асинхронных каналов // Для обработки сигналов Unix
                                    // Флаг для изящного завершения работы
    let shutdown = Arc::new(AtomicBool::new(false));

    // Настройка обработчика сигналов
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    let server = Server::new(
        PathBuf::from(APPS_DIR_ABS_PATH)
            .join(APP_DIR)
            .join(CHUNKS_DIR),
    )
    .await?;

    // Уведомляем systemd, что сервер готов к работе
    notify_systemd_ready()?;

    // Настройка watchdog
    setup_watchdog(Arc::clone(&shutdown)).await;

    // Клонирование сокета и флага для использования в асинхронном потоке
    let shutdown_clone = Arc::clone(&shutdown);
    let (tx, rx) = channel(100);

    // Запуск обработчика пакетов
    let handler = server.run(rx, shutdown_clone)?;
    let socket = server.get_socket_clone();

    // Основной цикл, который также проверяет сигналы для корректного завершения
    let receiver_task = tokio::spawn(async move {
        while !shutdown.load(Ordering::Relaxed) {
            select! {
                _ = socket.recv(&tx) => {
                    // Обработка полученных данных продолжается
                }
                _ = sigterm.recv() => {
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }
                _ = sigint.recv() => {
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }
                _ = sighup.recv() => {
                    // Здесь могла бы быть логика перезагрузки конфигурации
                }
            }
        }
    });

    // Ожидаем завершения любой из задач
    select! {
        _ = handler => {
            println!("Packet handler finished");
        }
        _ = receiver_task => {
            println!("Receiver task finished");
        }
    }

    // Уведомляем systemd о завершении работы
    if let Err(e) = notify_systemd_stopping() {
        eprintln!("Failed to notify systemd about stopping: {}", e);
    }

    Ok(())
}

#[tokio::main]
#[cfg(target_os = "windows")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows_service::service_dispatcher;
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        return Err(e.into());
    }

    Ok(())
}
