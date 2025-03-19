#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

// Общий код для обеих платформ
mod server;
mod socket;
mod stor;

#[tokio::main]
async fn main() {
    // Инициализируем логирование
    env_logger::init();

    // Вызываем соответствующую функцию запуска сервиса
    #[cfg(target_os = "linux")]
    {
        linux::run_service().await;
    }

    #[cfg(target_os = "windows")]
    {
        windows::run_service().await;
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        eprintln!("Эта платформа не поддерживается");
        std::process::exit(1);
    }
}
