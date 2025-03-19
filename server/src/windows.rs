use crate::server::AsyncServer;
use log::{error, info};
use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

#[cfg(target_os = "windows")]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher, Result,
};

// Константы для службы
const SERVICE_NAME: &str = "Leaf-server";

#[cfg(target_os = "windows")]
define_windows_service!(ffi_service_main, windows_service_main);

pub async fn run_service() {
    // Запуск Windows-службы
    #[cfg(target_os = "windows")]
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        error!("Ошибка запуска службы: {}", e);
    }
}

#[cfg(target_os = "windows")]
fn windows_service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_windows_service(arguments) {
        error!("Ошибка службы: {}", e);
    }
}

#[cfg(target_os = "windows")]
fn run_windows_service(_arguments: Vec<OsString>) -> Result<()> {
    // Создаем канал для коммуникации
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Определяем обработчик событий
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Регистрируем обработчик
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Сообщаем, что служба запущена
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Создаем токио рантайм внутри службы
    let rt = Runtime::new().expect("Не удалось создать токио рантайм");

    // Создаем oneshot канал для асинхронного завершения
    let (async_shutdown_tx, async_shutdown_rx) = oneshot::channel();

    // Запускаем асинхронный сервер в отдельной задаче
    rt.block_on(async {
        let server = AsyncServer::new();
        let shutdown_signal = server.get_shutdown_signal();

        // Запускаем сервер
        tokio::spawn(async move {
            server.run().await;
        });

        // Ожидаем сигнал завершения от SCM
        tokio::spawn(async move {
            // Используем стандартный канал для получения уведомления от SCM
            let std_rx = shutdown_rx;
            loop {
                match std_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // Получен сигнал завершения
                        shutdown_signal.store(true, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Продолжаем ожидание
                    }
                }
            }

            // Уведомляем основной поток о завершении
            let _ = async_shutdown_tx.send(());
        });

        // Ожидаем завершения
        let _ = async_shutdown_rx.await;
    });

    // Сообщаем, что служба остановлена
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
