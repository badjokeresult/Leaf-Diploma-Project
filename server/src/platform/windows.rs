use log::{error, info, warn};
use std::error::Error;
use std::ffi::OsString;
use std::sync::mpsc;
use tokio::sync::oneshot;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_NAME: &str = "RustService";
const SERVICE_DISPLAY_NAME: &str = "Rust Service";
const SERVICE_DESCRIPTION: &str = "Серверное приложение на Rust";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

// Функция для установки службы
pub fn install_service(_path: &str) -> Result<(), Box<dyn Error>> {
    // Путь игнорируется на Windows, используется только для Linux
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)?;

    let service_binary_path = std::env::current_exe()?;

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: SERVICE_TYPE,
        start_type: windows_service::service::ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None, // LocalSystem account
        account_password: None,
    };

    let service = manager.create_service(
        &service_info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
    )?;

    service.set_description(SERVICE_DESCRIPTION)?;

    info!("Служба Windows успешно установлена");

    // Попытка запустить службу
    match service.start(&[] as &[&str]) {
        Ok(_) => info!("Служба успешно запущена"),
        Err(e) => warn!("Не удалось запустить службу: {}", e),
    }

    Ok(())
}

// Функция для удаления службы
pub fn uninstall_service() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::DELETE | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;

    // Пробуем остановить службу, если она запущена
    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Stopped {
        info!("Останавливаем службу перед удалением...");
        service.stop()?;

        // Ждем пока служба остановится
        let mut status = service_status;
        while status.current_state != ServiceState::Stopped {
            std::thread::sleep(std::time::Duration::from_secs(1));
            status = service.query_status()?;
        }
    }

    // Удаляем службу
    service.delete()?;
    info!("Служба Windows успешно удалена");

    Ok(())
}

// Функция для запуска службы
pub fn start_service() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(SERVICE_NAME, ServiceAccess::START)?;

    service.start(&[] as &[&str])?;
    info!("Служба Windows успешно запущена");

    Ok(())
}

// Функция для остановки службы
pub fn stop_service() -> Result<(), Box<dyn Error>> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(SERVICE_NAME, ServiceAccess::STOP)?;

    service.stop()?;
    info!("Служба Windows успешно остановлена");

    Ok(())
}

// Определяем функцию обработчика службы Windows
define_windows_service!(ffi_service_main, service_main);

// Глобальная переменная для отправки сигнала завершения
static mut SHUTDOWN_TX: Option<std::sync::mpsc::Sender<()>> = None;

// Запуск приложения как службы Windows
pub fn run_as_service() -> Result<(), Box<dyn Error>> {
    // Пытаемся запуститься как служба
    match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

// Главная функция службы
fn service_main(_arguments: Vec<OsString>) {
    // Создаем канал для сигнала завершения
    let (tx, rx) = mpsc::channel();

    // Сохраняем отправителя в глобальной переменной
    unsafe {
        SHUTDOWN_TX = Some(tx);
    }

    // Регистрируем обработчик управления службой
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                info!("Получена команда остановки службы");

                // Отправляем сигнал на завершение
                unsafe {
                    if let Some(tx) = &SHUTDOWN_TX {
                        let _ = tx.send(());
                    }
                }

                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(e) => {
            error!(
                "Не удалось зарегистрировать обработчик событий службы: {}",
                e
            );
            return;
        }
    };

    // Устанавливаем статус "запускается"
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    }) {
        error!("Не удалось установить статус службы: {}", e);
        return;
    }

    // Создаем токио рантайм
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Не удалось создать токио рантайм: {}", e);
            return;
        }
    };

    // Запускаем основной код сервера
    let (service_shutdown_tx, service_shutdown_rx) = oneshot::channel();

    // Преобразуем mpsc канал в oneshot канал через отдельную задачу
    let shutdown_task = runtime.spawn(async move {
        match rx.recv() {
            Ok(_) => {
                let _ = service_shutdown_tx.send(());
            }
            Err(e) => {
                error!("Ошибка приема сигнала завершения: {}", e);
            }
        }
    });

    // Запускаем основную логику сервера
    info!("Запуск основной логики сервера в службе Windows");
    match runtime.block_on(async {
        if let Err(e) = server::run(service_shutdown_rx).await {
            error!("Ошибка выполнения сервера: {}", e);
        }
    }) {
        _ => info!("Сервер завершил работу"),
    }

    // Отменяем задачу мониторинга сигналов
    shutdown_task.abort();

    // Устанавливаем статус "остановлена"
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    }) {
        error!("Не удалось установить статус службы: {}", e);
    }
}

// Настройка обработчика сигналов для корректного завершения
pub fn setup_signal_handler(shutdown_sender: oneshot::Sender<()>) -> Result<(), Box<dyn Error>> {
    // В Windows используем Ctrl+C для завершения в консольном режиме
    let signal_handle = tokio::spawn(async move {
        if let Ok(_) = tokio::signal::ctrl_c().await {
            info!("Получен сигнал Ctrl+C");
            let _ = shutdown_sender.send(());
        }
    });

    Ok(())
}
