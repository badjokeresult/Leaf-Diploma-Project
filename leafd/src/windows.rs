use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::runtime::Runtime;

use anyhow::Result;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

use consts::*;

mod consts {
    pub const SERVICE_NAME: &str = "LeafServer";
    pub const APPS_DIR_ABS_PATH: &str = "C:\\Program Files";
    pub const APP_DIR: &str = "Leaf";
    pub const CHUKS_DIR: &str = "Chunks";
}

define_windows_service!(ffi_service_main, service_main);

// Главная функция службы, запускаемая диспетчером
pub fn service_main(_arguments: Vec<OsString>) {
    // Создаем канал для коммуникации между обработчиком событий и основным потоком
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Создаем обработчик событий управления службой
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                info!("Получен сигнал остановки службы, завершаем работу...");
                if let Err(e) = shutdown_tx.send(()) {
                    error!("Ошибка при отправке сигнала завершения: {}", e);
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Регистрируем обработчик
    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(e) => {
            error!("Ошибка при регистрации обработчика службы: {}", e);
            return;
        }
    };

    // Сообщаем системе, что служба запущена
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        return;
    }

    // Запускаем UDP сервер в асинхронном режиме
    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            return;
        }
    };

    let server = runtime
        .spawn(
            Server::new(
                PathBuf::from(APPS_DIR_ABS_PATH)
                    .join(APP_DIR)
                    .join(CHUNKS_DIR),
            )
            .await
            .unwrap(),
        )
        .unwrap();
    let (tx, rx) = runtime
        .spawn(tokio::sync::mpsc::channel(100).await.unwrap())
        .unwrap();
    let sh_rx = Arc::new(AtomicBool);
    let server_handle = runtime.spawn(server.run(rx, sh_rx));
    let socket = server.get_socket_clone();
    let socket_handle = runtime.spawn(socket.recv(&tx));

    // Ожидаем сигнала завершения
    let _ = shutdown_rx.recv();

    // Сообщаем системе, что служба останавливается
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    }) {}

    // Закрываем runtime, что приведет к остановке UDP сервера
    drop(runtime);

    // Сообщаем системе, что служба остановлена
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {}
}
