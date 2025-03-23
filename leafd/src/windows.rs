use std::ffi::OsString; // Тип C-строк
use std::path::PathBuf; // Тип файлового пути
use std::sync::mpsc; // Модуль каналов
use std::time::Duration; // Тип времени ожидания

use tokio::net::UdpSocket; // Асинхронный UDP-сокет
use tokio::runtime::Runtime; // Среда асинхронного исполнения

use anyhow::Result; // Корректный тип результата
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
}; // Зависимости для работы со службами Windows

use consts::*; // Модуль с константами

mod consts {
    pub const SERVICE_NAME: &str = "LeafServer"; // Имя будущей службы
    pub const APPS_DIR_ABS_PATH: &str = "C:\\Program Files"; // Корень директории с приложениями
    pub const APP_DIR: &str = "Leaf"; // Корень приложения
    pub const CHUNKS_DIR: &str = "Chunks"; // Директория хранилища
}

define_windows_service!(ffi_service_main, service_main); // Определение новой службы

// Главная функция службы, запускаемая диспетчером
pub fn service_main(_arguments: Vec<OsString>) {
    // Создаем канал для коммуникации между обработчиком событий и основным потоком
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Создаем обработчик событий управления службой
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                if let Err(e) = shutdown_tx.send(()) {
                    eprintln!("Ошибка при отправке сигнала завершения: {}", e);
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
            eprintln!("Ошибка при регистрации обработчика службы: {}", e);
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

    let runtime = match Runtime::new() {
        // Создаем среду асинхронного исполнения
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
        .unwrap(); // В ней создается сервер
    let (tx, rx) = runtime
        .spawn(tokio::sync::mpsc::channel(100).await.unwrap())
        .unwrap(); // В ней же создаем асинхронный канал
    let sh_rx = Arc::new(AtomicBool); // Создаем флаг завершения
    let server_handle = runtime.spawn(server.run(rx, sh_rx)); // Запускаем сервер
    let socket = server.get_socket_clone(); // Получаем сокет
    let socket_handle = runtime.spawn(socket.recv(&tx)); // Отдельно запускаем получение пакетов

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
