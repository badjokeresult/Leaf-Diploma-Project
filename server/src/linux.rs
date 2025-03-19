use crate::server::AsyncServer;
use daemonize::Daemonize;
use log::{error, info};
use std::fs::File;
use tokio::signal::unix::{signal, SignalKind};

pub async fn run_service() {
    let stdout = File::create("/tmp/leaf.out").unwrap();
    let stderr = File::create("/tmp/leaf.err").unwrap();

    let daemonize = Daemonize::new()
        .pid_file("/tmp/leaf.pid")
        .chown_pid_file(true)
        .working_directory("/tmp")
        .user("leaf-client")
        .group("leaf-client")
        .umask(0o777)
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => {
            service_main_loop().await;
        }
        Err(e) => error!("Ошибка запуска демона: {}", e),
    }
}

async fn service_main_loop() {
    // Создаем экземпляр сервера
    let server = AsyncServer::new();
    server.get_shutdown_signal();

    // Настраиваем обработку сигналов SIGTERM и SIGINT
    let mut sigterm =
        signal(SignalKind::terminate()).expect("Не удалось настроить обработку SIGTERM");
    let mut sigint =
        signal(SignalKind::interrupt()).expect("Не удалось настроить обработку SIGINT");

    // Запускаем сервер и обработку сигналов в отдельных тасках
    tokio::select! {
        _ = server.run() => {
            info!("Сервер завершил работу");
        }
        _ = sigterm.recv() => {
            info!("Получен сигнал SIGTERM");
            server.shutdown();
        }
        _ = sigint.recv() => {
            info!("Получен сигнал SIGINT");
            server.shutdown();
        }
    }

    info!("Linux-служба завершена");
}
